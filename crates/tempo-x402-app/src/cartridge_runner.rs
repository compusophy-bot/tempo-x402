//! Client-side WASM cartridge runner — WASM within WASM.
//!
//! Fetches a cartridge `.wasm` binary from the server, instantiates it
//! in the browser via `js_sys::WebAssembly`, provides the x402 host ABI
//! as import closures, and captures the cartridge's response output.
//!
//! This is the browser-side equivalent of `tempo-x402-cartridge::engine`.
//! The Leptos SPA IS the cartridge runtime.

use std::cell::RefCell;
use std::rc::Rc;

use js_sys::{Function, Object, Reflect, Uint8Array, WebAssembly};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

/// Result of running a cartridge client-side.
#[derive(Debug, Clone)]
pub struct CartridgeOutput {
    pub status: u16,
    pub body: String,
    pub content_type: String,
    pub logs: Vec<String>,
}

/// Fetch and run a cartridge in the browser.
///
/// 1. Fetches `/c/{slug}/wasm` binary
/// 2. Instantiates with x402 ABI imports
/// 3. Calls `x402_handle` with a GET request
/// 4. Captures response output
pub async fn run_cartridge(slug: &str) -> Result<CartridgeOutput, String> {
    // 1. Fetch the WASM binary
    let window = web_sys::window().ok_or("no window")?;
    let url = format!("/c/{slug}/wasm");
    let resp: web_sys::Response = JsFuture::from(window.fetch_with_str(&url))
        .await
        .map_err(|e| format!("fetch failed: {e:?}"))?
        .dyn_into()
        .map_err(|_| "response cast failed")?;

    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let array_buffer = JsFuture::from(
        resp.array_buffer()
            .map_err(|e| format!("array_buffer: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("await array_buffer: {e:?}"))?;
    let wasm_bytes = Uint8Array::new(&array_buffer).to_vec();

    if wasm_bytes.is_empty() {
        return Err("empty WASM binary".to_string());
    }

    // 2. Build shared state for capturing output
    let output = Rc::new(RefCell::new(CartridgeOutput {
        status: 200,
        body: String::new(),
        content_type: "text/html".to_string(),
        logs: Vec::new(),
    }));
    let memory_ref: Rc<RefCell<Option<WebAssembly::Memory>>> = Rc::new(RefCell::new(None));

    // 3. Build the x402 import namespace
    let imports = build_imports(output.clone(), memory_ref.clone())?;

    // 4. Instantiate
    let result = JsFuture::from(WebAssembly::instantiate_buffer(&wasm_bytes, &imports))
        .await
        .map_err(|e| format!("instantiate failed: {e:?}"))?;

    let instance: WebAssembly::Instance = Reflect::get(&result, &"instance".into())
        .map_err(|e| format!("get instance: {e:?}"))?
        .dyn_into()
        .map_err(|_| "instance cast failed")?;

    let exports = instance.exports();

    // 5. Capture the child's memory
    let mem: WebAssembly::Memory = Reflect::get(exports.as_ref(), &"memory".into())
        .map_err(|e| format!("get memory: {e:?}"))?
        .dyn_into()
        .map_err(|_| "memory cast failed")?;
    *memory_ref.borrow_mut() = Some(mem.clone());

    // 6. Write request JSON into guest memory via x402_alloc
    let request_json = r#"{"method":"GET","path":"/","body":"","headers":{}}"#;
    let request_bytes = request_json.as_bytes();

    let alloc_fn = Reflect::get(exports.as_ref(), &"x402_alloc".into())
        .ok()
        .and_then(|f| f.dyn_into::<Function>().ok());

    let (req_ptr, req_len) = if let Some(alloc) = alloc_fn {
        let ptr_val = alloc
            .call1(
                &JsValue::undefined(),
                &JsValue::from(request_bytes.len() as i32),
            )
            .map_err(|e| format!("alloc failed: {e:?}"))?;
        let ptr = ptr_val.as_f64().unwrap_or(0.0) as u32;

        // Write request bytes into guest memory
        let buffer = mem.buffer();
        let view =
            Uint8Array::new_with_byte_offset_and_length(&buffer, ptr, request_bytes.len() as u32);
        view.copy_from(request_bytes);
        (ptr as i32, request_bytes.len() as i32)
    } else {
        // No allocator — write at offset 0 (risky but works for simple cartridges)
        let buffer = mem.buffer();
        let view =
            Uint8Array::new_with_byte_offset_and_length(&buffer, 0, request_bytes.len() as u32);
        view.copy_from(request_bytes);
        (0i32, request_bytes.len() as i32)
    };

    // 7. Call x402_handle
    let handle_fn = Reflect::get(exports.as_ref(), &"x402_handle".into())
        .map_err(|e| format!("no x402_handle export: {e:?}"))?
        .dyn_into::<Function>()
        .map_err(|_| "x402_handle is not a function")?;

    handle_fn
        .call2(
            &JsValue::undefined(),
            &JsValue::from(req_ptr),
            &JsValue::from(req_len),
        )
        .map_err(|e| format!("x402_handle failed: {e:?}"))?;

    // 8. Return captured output
    let result = output.borrow().clone();
    Ok(result)
}

/// Read a UTF-8 string from a WebAssembly.Memory at (ptr, len).
fn read_guest_string(mem: &WebAssembly::Memory, ptr: i32, len: i32) -> String {
    if len <= 0 {
        return String::new();
    }
    let buffer = mem.buffer();
    let view = Uint8Array::new_with_byte_offset_and_length(&buffer, ptr as u32, len as u32);
    let mut bytes = vec![0u8; len as usize];
    view.copy_to(&mut bytes);
    String::from_utf8_lossy(&bytes).to_string()
}

/// Build the x402 import object with Closure-backed host functions.
fn build_imports(
    output: Rc<RefCell<CartridgeOutput>>,
    memory_ref: Rc<RefCell<Option<WebAssembly::Memory>>>,
) -> Result<Object, String> {
    let imports = Object::new();

    // Build both "x402" (correct) and "env" (backward-compat) namespaces
    let x402_ns = Object::new();
    let env_ns = Object::new();

    // ── log ──
    let log_output = output.clone();
    let log_mem = memory_ref.clone();
    let log_closure = wasm_bindgen::closure::Closure::<dyn Fn(i32, i32, i32)>::new(
        move |_level: i32, ptr: i32, len: i32| {
            let msg = if let Some(ref mem) = *log_mem.borrow() {
                read_guest_string(mem, ptr, len)
            } else {
                format!("[no memory] ptr={ptr} len={len}")
            };
            log_output.borrow_mut().logs.push(msg);
        },
    );
    let log_fn: &JsValue = log_closure.as_ref().unchecked_ref();
    Reflect::set(&x402_ns, &"log".into(), log_fn).map_err(|e| format!("{e:?}"))?;
    Reflect::set(&env_ns, &"x402_log".into(), log_fn).map_err(|e| format!("{e:?}"))?;
    log_closure.forget();

    // ── response ──
    let resp_output = output.clone();
    let resp_mem = memory_ref.clone();
    let response_closure = wasm_bindgen::closure::Closure::<dyn Fn(i32, i32, i32, i32, i32)>::new(
        move |status: i32, body_ptr: i32, body_len: i32, ct_ptr: i32, ct_len: i32| {
            if let Some(ref mem) = *resp_mem.borrow() {
                let body = read_guest_string(mem, body_ptr, body_len);
                let ct = read_guest_string(mem, ct_ptr, ct_len);
                let mut out = resp_output.borrow_mut();
                out.status = status as u16;
                out.body = body;
                if !ct.is_empty() {
                    out.content_type = ct;
                }
            }
        },
    );
    let resp_fn: &JsValue = response_closure.as_ref().unchecked_ref();
    Reflect::set(&x402_ns, &"response".into(), resp_fn).map_err(|e| format!("{e:?}"))?;
    Reflect::set(&env_ns, &"x402_response".into(), resp_fn).map_err(|e| format!("{e:?}"))?;
    response_closure.forget();

    // ── kv_get (stub — returns 0 = not found) ──
    let kv_get_closure =
        wasm_bindgen::closure::Closure::<dyn Fn(i32, i32) -> f64>::new(|_ptr: i32, _len: i32| 0.0);
    let kv_get_fn: &JsValue = kv_get_closure.as_ref().unchecked_ref();
    Reflect::set(&x402_ns, &"kv_get".into(), kv_get_fn).map_err(|e| format!("{e:?}"))?;
    Reflect::set(&env_ns, &"x402_kv_get".into(), kv_get_fn).map_err(|e| format!("{e:?}"))?;
    kv_get_closure.forget();

    // ── kv_set (stub — no-op) ──
    let kv_set_closure = wasm_bindgen::closure::Closure::<dyn Fn(i32, i32, i32, i32) -> i32>::new(
        |_: i32, _: i32, _: i32, _: i32| 0,
    );
    let kv_set_fn: &JsValue = kv_set_closure.as_ref().unchecked_ref();
    Reflect::set(&x402_ns, &"kv_set".into(), kv_set_fn).map_err(|e| format!("{e:?}"))?;
    Reflect::set(&env_ns, &"x402_kv_set".into(), kv_set_fn).map_err(|e| format!("{e:?}"))?;
    kv_set_closure.forget();

    // ── payment_info (stub — returns 0) ──
    let payment_closure = wasm_bindgen::closure::Closure::<dyn Fn() -> f64>::new(|| 0.0);
    let payment_fn: &JsValue = payment_closure.as_ref().unchecked_ref();
    Reflect::set(&x402_ns, &"payment_info".into(), payment_fn).map_err(|e| format!("{e:?}"))?;
    Reflect::set(&env_ns, &"x402_payment_info".into(), payment_fn).map_err(|e| format!("{e:?}"))?;
    payment_closure.forget();

    // Attach namespaces
    Reflect::set(&imports, &"x402".into(), &x402_ns).map_err(|e| format!("{e:?}"))?;
    Reflect::set(&imports, &"env".into(), &env_ns).map_err(|e| format!("{e:?}"))?;

    Ok(imports)
}

// ── Interactive Cartridge Runtime ──────────────────────────────────

/// Cartridge type — detected by checking exports after instantiation.
pub enum CartridgeType {
    /// Exports x402_handle — returns text/HTML/JSON responses.
    Backend,
    /// Exports x402_tick — framebuffer rendering at 60fps.
    Interactive,
}

/// A running interactive cartridge instance.
pub struct InteractiveCartridge {
    pub memory: WebAssembly::Memory,
    pub tick_fn: Function,
    pub key_down_fn: Option<Function>,
    pub key_up_fn: Option<Function>,
    pub fb_ptr_fn: Function,
    pub width: u32,
    pub height: u32,
}

/// Fetch a cartridge binary and detect its type.
pub async fn detect_type(slug: &str) -> Result<(CartridgeType, Vec<u8>), String> {
    let window = web_sys::window().ok_or("no window")?;
    let url = format!("/c/{slug}/wasm");
    let resp: web_sys::Response = JsFuture::from(window.fetch_with_str(&url))
        .await
        .map_err(|e| format!("fetch: {e:?}"))?
        .dyn_into()
        .map_err(|_| "response cast")?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let buf = JsFuture::from(resp.array_buffer().map_err(|e| format!("{e:?}"))?)
        .await
        .map_err(|e| format!("{e:?}"))?;
    let bytes = Uint8Array::new(&buf).to_vec();
    if bytes.is_empty() {
        return Err("empty binary".into());
    }

    // Quick instantiate to check exports
    let imports = build_imports(
        Rc::new(RefCell::new(CartridgeOutput {
            status: 200,
            body: String::new(),
            content_type: String::new(),
            logs: Vec::new(),
        })),
        Rc::new(RefCell::new(None)),
    )?;
    let result = JsFuture::from(WebAssembly::instantiate_buffer(&bytes, &imports))
        .await
        .map_err(|e| format!("instantiate: {e:?}"))?;
    let instance: WebAssembly::Instance = Reflect::get(&result, &"instance".into())
        .map_err(|e| format!("{e:?}"))?
        .dyn_into()
        .map_err(|_| "instance cast")?;
    let exports = instance.exports();

    let has_tick = Reflect::get(exports.as_ref(), &"x402_tick".into())
        .ok()
        .and_then(|v| v.dyn_into::<Function>().ok())
        .is_some();

    if has_tick {
        Ok((CartridgeType::Interactive, bytes))
    } else {
        Ok((CartridgeType::Backend, bytes))
    }
}

/// Instantiate an interactive cartridge for 60fps rendering.
pub async fn instantiate_interactive(
    wasm_bytes: &[u8],
    width: u32,
    height: u32,
) -> Result<InteractiveCartridge, String> {
    let output = Rc::new(RefCell::new(CartridgeOutput {
        status: 200,
        body: String::new(),
        content_type: String::new(),
        logs: Vec::new(),
    }));
    let memory_ref: Rc<RefCell<Option<WebAssembly::Memory>>> = Rc::new(RefCell::new(None));
    let imports = build_imports(output, memory_ref.clone())?;

    let result = JsFuture::from(WebAssembly::instantiate_buffer(wasm_bytes, &imports))
        .await
        .map_err(|e| format!("instantiate: {e:?}"))?;
    let instance: WebAssembly::Instance = Reflect::get(&result, &"instance".into())
        .map_err(|e| format!("{e:?}"))?
        .dyn_into()
        .map_err(|_| "instance cast")?;
    let exports = instance.exports();

    let memory: WebAssembly::Memory = Reflect::get(exports.as_ref(), &"memory".into())
        .map_err(|e| format!("memory: {e:?}"))?
        .dyn_into()
        .map_err(|_| "memory cast")?;
    *memory_ref.borrow_mut() = Some(memory.clone());

    let tick_fn = Reflect::get(exports.as_ref(), &"x402_tick".into())
        .map_err(|e| format!("x402_tick: {e:?}"))?
        .dyn_into::<Function>()
        .map_err(|_| "x402_tick not a function")?;

    let key_down_fn = Reflect::get(exports.as_ref(), &"x402_key_down".into())
        .ok()
        .and_then(|v| v.dyn_into::<Function>().ok());
    let key_up_fn = Reflect::get(exports.as_ref(), &"x402_key_up".into())
        .ok()
        .and_then(|v| v.dyn_into::<Function>().ok());
    let fb_ptr_fn = Reflect::get(exports.as_ref(), &"x402_get_framebuffer".into())
        .map_err(|e| format!("x402_get_framebuffer: {e:?}"))?
        .dyn_into::<Function>()
        .map_err(|_| "x402_get_framebuffer not a function")?;

    // Call x402_init if it exists
    if let Ok(init_fn) = Reflect::get(exports.as_ref(), &"x402_init".into())
        .and_then(|v| v.dyn_into::<Function>().map_err(|e| e.into()))
    {
        let _ = init_fn.call2(
            &JsValue::undefined(),
            &JsValue::from(width as i32),
            &JsValue::from(height as i32),
        );
    }

    Ok(InteractiveCartridge {
        memory,
        tick_fn,
        key_down_fn,
        key_up_fn,
        fb_ptr_fn,
        width,
        height,
    })
}

/// Read the RGBA framebuffer from the cartridge's WASM memory.
// ── Frontend Cartridge Loader ────────────────────────────────────

/// Load a frontend cartridge (Leptos app) by dynamically importing its JS glue
/// and calling init(selector) to mount it into a DOM element.
///
/// The cartridge's wasm-bindgen JS glue is at `/c/{slug}/pkg/{slug}.js`.
/// It exports `default(wasm_url)` to initialize WASM and `init(selector)` to mount.
pub async fn load_frontend_cartridge(slug: &str, mount_id: &str) -> Result<(), String> {
    // The slug may contain hyphens, but wasm-bindgen converts them to underscores in filenames
    let file_slug = slug.replace('-', "_");
    let js_url = format!("/c/{slug}/pkg/{file_slug}.js");
    let wasm_url = format!("/c/{slug}/pkg/{file_slug}_bg.wasm");

    // Use dynamic import() to load the JS glue module, then init WASM and mount.
    // Some cartridges export `init(selector)` (template pattern), others use
    // `#[wasm_bindgen(start)]` which auto-runs on load and call mount_to_body().
    // For the latter, we capture any new children added to <body> during init
    // and move them into the Studio mount div.
    let script = format!(
        r#"(async () => {{
            const mount = document.getElementById('{mount_id}');
            const bodyCountBefore = document.body.children.length;
            const mod = await import('{js_url}');
            await mod.default('{wasm_url}');
            if (typeof mod.init === 'function') {{
                mod.init('#{mount_id}');
            }} else if (mount) {{
                // wasm_bindgen(start) mounted to body — move new content into mount div
                while (document.body.children.length > bodyCountBefore) {{
                    mount.appendChild(document.body.children[bodyCountBefore]);
                }}
            }}
        }})()"#
    );

    let result = js_sys::eval(&script);
    match result {
        Ok(promise) => {
            // The eval returns a Promise — await it
            let _ = JsFuture::from(js_sys::Promise::from(promise))
                .await
                .map_err(|e| format!("frontend cartridge load failed: {e:?}"))?;
            Ok(())
        }
        Err(e) => Err(format!("eval failed: {e:?}")),
    }
}

pub fn read_framebuffer(cart: &InteractiveCartridge) -> Vec<u8> {
    let ptr_val = cart
        .fb_ptr_fn
        .call0(&JsValue::undefined())
        .unwrap_or(JsValue::from(0));
    let ptr = ptr_val.as_f64().unwrap_or(0.0) as u32;
    let size = cart.width * cart.height * 4;

    // MUST re-acquire buffer() each frame — it detaches after memory.grow()
    let buffer = cart.memory.buffer();
    let view = Uint8Array::new_with_byte_offset_and_length(&buffer, ptr, size);
    let mut pixels = vec![0u8; size as usize];
    view.copy_to(&mut pixels);
    pixels
}
