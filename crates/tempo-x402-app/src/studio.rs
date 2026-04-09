//! Studio — unified app workspace for building, previewing, and chatting with the AI agent.
//!
//! Three-panel layout: Cartridges/Files (left) | Preview/Editor (center) | Chat (right)
//! Status bar at bottom shows intelligence metrics. Mobile collapses to single-panel with drawer.

use std::cell::RefCell;
use std::rc::Rc;

use leptos::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::api;

/// App entry — a script endpoint or WASM cartridge.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct AppEntry {
    slug: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    kind: String,
    /// "backend", "interactive", or "frontend"
    #[serde(default)]
    cartridge_type: String,
}

/// File entry from the workspace.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct FileEntry {
    name: String,
    #[serde(rename = "type")]
    entry_type: String,
    size: Option<u64>,
}

/// Chat message with tool execution visibility.
#[derive(Clone, Debug)]
struct ChatMsg {
    role: String,
    content: String,
    tools: Vec<serde_json::Value>,
}

/// What the center panel is showing.
#[derive(Clone, Debug, PartialEq)]
enum CenterView {
    Welcome,
    AppPreview(String),
    CartridgePreview(String),
    #[allow(dead_code)]
    InteractivePreview(String),
    FrontendPreview(String),
    FileView(String, String),
}

/// Studio page — the unified app workspace.
#[component]
pub fn StudioPage() -> impl IntoView {
    // ── Core state ──
    let (apps, set_apps) = create_signal(Vec::<AppEntry>::new());
    let (center, set_center) = create_signal(CenterView::Welcome);
    let (messages, set_messages) = create_signal(Vec::<ChatMsg>::new());
    let (input, set_input) = create_signal(String::new());
    let (sending, set_sending) = create_signal(false);
    let (session_id, set_session_id) = create_signal(None::<String>);
    let (soul_status, set_soul_status) = create_signal(None::<serde_json::Value>);
    let (sys_metrics, set_sys_metrics) = create_signal(None::<serde_json::Value>);
    let (file_tree, set_file_tree) = create_signal(Vec::<FileEntry>::new());
    let (current_path, set_current_path) = create_signal("crates".to_string());
    let (files_expanded, set_files_expanded) = create_signal(false);
    let (file_error, set_file_error) = create_signal(None::<String>);
    let (sidebar_open, set_sidebar_open) = create_signal(false);
    let messages_ref = create_node_ref::<html::Div>();

    // ── Scroll to bottom ──
    let scroll_bottom = move || {
        request_animation_frame(move || {
            if let Some(el) = messages_ref.get() {
                el.set_scroll_top(el.scroll_height());
            }
        });
    };

    // ── Fetch apps ──
    let refresh_apps = move || {
        spawn_local(async move {
            let mut all_apps = Vec::new();

            if let Ok(data) = api::fetch_json("/x").await {
                if let Some(eps) = data.get("endpoints").and_then(|v| v.as_array()) {
                    for ep in eps {
                        let slug = ep
                            .get("slug")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let desc = ep
                            .get("description")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        if !slug.is_empty() {
                            all_apps.push(AppEntry {
                                slug,
                                description: desc,
                                kind: "script".into(),
                                cartridge_type: String::new(),
                            });
                        }
                    }
                }
            }

            if let Ok(data) = api::fetch_json("/c").await {
                if let Some(carts) = data.get("cartridges").and_then(|v| v.as_array()) {
                    for c in carts {
                        let slug = c
                            .get("slug")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let desc = c
                            .get("description")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        let ct = c
                            .get("cartridge_type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("backend")
                            .to_string();
                        if !slug.is_empty() {
                            all_apps.push(AppEntry {
                                slug,
                                description: desc,
                                kind: "cartridge".into(),
                                cartridge_type: ct,
                            });
                        }
                    }
                }
            }

            set_apps.set(all_apps);
        });
    };
    refresh_apps();

    // ── Fetch status (once on load, then after each chat) ──
    let refresh_status = move || {
        spawn_local(async move {
            if let Ok(data) = api::fetch_soul_status().await {
                set_soul_status.set(Some(data));
            }
        });
        spawn_local(async move {
            if let Ok(data) = api::fetch_json("/soul/system").await {
                set_sys_metrics.set(Some(data));
            }
        });
    };
    refresh_status();

    // ── New conversation ──
    let new_conversation = move |_| {
        set_session_id.set(None);
        set_messages.set(Vec::new());
    };

    // ── Send chat ──
    let send_message = move || {
        let msg = input.get_untracked();
        if msg.trim().is_empty() || sending.get_untracked() {
            return;
        }
        set_sending.set(true);
        set_input.set(String::new());

        set_messages.update(|msgs| {
            msgs.push(ChatMsg {
                role: "user".into(),
                content: msg.clone(),
                tools: vec![],
            });
        });
        scroll_bottom();

        let sid = session_id.get_untracked();
        let refresh = refresh_apps;
        spawn_local(async move {
            match api::send_soul_chat(&msg, sid.as_deref()).await {
                Ok(resp) => {
                    let reply = resp
                        .get("reply")
                        .and_then(|v| v.as_str())
                        .unwrap_or("(no response)")
                        .to_string();
                    if let Some(new_sid) = resp.get("session_id").and_then(|v| v.as_str()) {
                        set_session_id.set(Some(new_sid.to_string()));
                    }
                    let tools = resp
                        .get("tool_executions")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    set_messages.update(|msgs| {
                        msgs.push(ChatMsg {
                            role: "assistant".into(),
                            content: reply,
                            tools,
                        });
                    });
                    // Refresh apps if tools modified endpoints
                    let modified = resp
                        .get("tool_executions")
                        .and_then(|v| v.as_array())
                        .map(|execs| {
                            execs.iter().any(|e| {
                                let cmd = e.get("command").and_then(|v| v.as_str()).unwrap_or("");
                                cmd.contains("create_script")
                                    || cmd.contains("delete_endpoint")
                                    || cmd.contains("create_cartridge")
                                    || cmd.contains("compile_cartridge")
                                    || cmd.contains("delete_cartridge")
                            })
                        })
                        .unwrap_or(false);
                    if modified {
                        refresh();
                    }
                }
                Err(e) => {
                    set_messages.update(|msgs| {
                        msgs.push(ChatMsg {
                            role: "assistant".into(),
                            content: format!("Error: {e}"),
                            tools: vec![],
                        });
                    });
                }
            }
            set_sending.set(false);
            refresh_status();
            scroll_bottom();
        });
    };

    let send_for_key = send_message;
    let on_keydown = move |ev: web_sys::KeyboardEvent| {
        if ev.key() == "Enter" && !ev.shift_key() {
            ev.prevent_default();
            send_for_key();
        }
    };

    // ── File browser ──
    let load_tree = move |path: String| {
        spawn_local(async move {
            set_current_path.set(path.clone());
            set_file_error.set(None);
            match fetch_file_tree(&path).await {
                Ok(files) => set_file_tree.set(files),
                Err(e) => {
                    set_file_tree.set(vec![]);
                    set_file_error.set(Some(e));
                }
            }
        });
    };

    let load_file = move |path: String| {
        spawn_local(async move {
            if let Ok(content) = fetch_file_content(&path).await {
                set_center.set(CenterView::FileView(path, content));
            }
        });
    };

    // ── Delete cartridge ──
    let delete_app = move |slug: String, kind: String| {
        let refresh = refresh_apps;
        spawn_local(async move {
            if kind == "cartridge" {
                let _ = api::delete_cartridge(&slug).await;
            } else {
                let _ =
                    gloo_net::http::Request::delete(&format!("/admin/endpoints/script-{}", slug))
                        .send()
                        .await;
            }
            refresh();
        });
    };

    view! {
        <div class="studio">
            // ── Header ──
            <div class="studio-header">
                <div class="studio-header-left">
                    <button class="studio-mobile-toggle" on:click=move |_| set_sidebar_open.update(|v| *v = !*v)>
                        {move || if sidebar_open.get() { "\u{2715}" } else { "\u{2630}" }}
                    </button>
                    <h2>"Studio"</h2>
                    <button class="btn btn-sm" on:click=new_conversation>"+ New Chat"</button>
                </div>
                <div class="studio-header-right">
                    {move || {
                        let s = soul_status.get();
                        let mode = s.as_ref().and_then(|d| d.get("mode")).and_then(|v| v.as_str()).unwrap_or("--").to_string();
                        let coding = s.as_ref().and_then(|d| d.get("coding_enabled")).and_then(|v| v.as_bool()).unwrap_or(false);
                        let iq = s.as_ref().and_then(|d| d.get("benchmark")).and_then(|b| b.get("opus_iq")).and_then(|v| v.as_str()).unwrap_or("--").to_string();
                        view! {
                            <span class="studio-badge">{mode}</span>
                            <span class="studio-badge">{if coding { "coding" } else { "read-only" }}</span>
                            <span class="studio-badge">"IQ "{iq}</span>
                        }
                    }}
                </div>
            </div>

            // ── Three-panel layout ──
            <div class="studio-layout">

                // ── Left: Sidebar ──
                <div class="studio-sidebar" class:open=move || sidebar_open.get()>
                    <div class="studio-section">
                        <div class="studio-section-header">
                            <span>"Cartridges"</span>
                        </div>
                        {move || {
                            let app_list = apps.get();
                            if app_list.is_empty() {
                                view! {
                                    <div class="studio-empty">
                                        <p>"No cartridges yet"</p>
                                        <p class="studio-hint">"Ask the chat to create one"</p>
                                    </div>
                                }.into_view()
                            } else {
                                view! {
                                    <div class="studio-app-list">
                                        {app_list.iter().map(|app| {
                                            let slug = app.slug.clone();
                                            let kind = app.kind.clone();
                                            let desc = app.description.clone().unwrap_or_default();
                                            let slug_click = slug.clone();
                                            let slug_del = slug.clone();
                                            let kind_del = kind.clone();
                                            let kind_for_click = kind.clone();
                                            let ct_for_click = app.cartridge_type.clone();
                                            view! {
                                                <div
                                                    class="studio-app-item"
                                                    on:click=move |_| {
                                                        set_sidebar_open.set(false);
                                                        if ct_for_click == "frontend" {
                                                            set_center.set(CenterView::FrontendPreview(slug_click.clone()));
                                                        } else if kind_for_click == "cartridge" {
                                                            set_center.set(CenterView::CartridgePreview(slug_click.clone()));
                                                        } else {
                                                            set_center.set(CenterView::AppPreview(slug_click.clone()));
                                                        }
                                                    }
                                                >
                                                    <span class="studio-app-name">{&slug}</span>
                                                    <span class="studio-app-badge">{&kind}</span>
                                                    <button class="studio-app-delete" on:click=move |ev: web_sys::MouseEvent| {
                                                        ev.stop_propagation();
                                                        delete_app(slug_del.clone(), kind_del.clone());
                                                    } title="Delete">{"\u{00D7}"}</button>
                                                    {(!desc.is_empty()).then(|| view! {
                                                        <span class="studio-app-desc">{&desc}</span>
                                                    })}
                                                </div>
                                            }
                                        }).collect_view()}
                                    </div>
                                }.into_view()
                            }
                        }}
                    </div>

                    // Files (collapsible)
                    <div class="studio-section">
                        <div
                            class="studio-section-header studio-section-toggle"
                            on:click=move |_| {
                                let expanded = !files_expanded.get_untracked();
                                set_files_expanded.set(expanded);
                                if expanded { load_tree("crates".to_string()); }
                            }
                        >
                            {move || if files_expanded.get() { "Files \u{25BE}" } else { "Files \u{25B8}" }}
                        </div>
                        {move || {
                            if !files_expanded.get() {
                                return view! { <span></span> }.into_view();
                            }
                            view! {
                                <div class="studio-file-path">{move || current_path.get()}</div>
                                {move || file_error.get().map(|e| view! {
                                    <div class="studio-file-error">{e}</div>
                                })}
                                <div class="studio-file-list">
                                    {move || {
                                        let path = current_path.get();
                                        if path != "crates" && path.contains('/') {
                                            let parent = path.rsplit_once('/').map(|(p, _)| p.to_string()).unwrap_or_else(|| "crates".to_string());
                                            Some(view! {
                                                <div class="studio-file studio-file--dir" on:click=move |_| load_tree(parent.clone())>
                                                    <span>"\u{2190} .."</span>
                                                </div>
                                            })
                                        } else {
                                            None
                                        }
                                    }}
                                    {move || {
                                        file_tree.get().iter().map(|entry| {
                                            let name = entry.name.clone();
                                            let is_dir = entry.entry_type == "directory";
                                            let full_path = format!("{}/{}", current_path.get_untracked(), name);
                                            let path_for_click = full_path.clone();
                                            view! {
                                                <div
                                                    class=if is_dir { "studio-file studio-file--dir" } else { "studio-file" }
                                                    on:click=move |_| {
                                                        if is_dir { load_tree(path_for_click.clone()); }
                                                        else { load_file(path_for_click.clone()); }
                                                    }
                                                >
                                                    <span>{if is_dir { "\u{1F4C1} " } else { "" }}</span>
                                                    <span>{&name}</span>
                                                </div>
                                            }
                                        }).collect_view()
                                    }}
                                </div>
                            }.into_view()
                        }}
                    </div>
                </div>

                // ── Center: Preview ──
                <div class="studio-center">
                    {move || {
                        match center.get() {
                            CenterView::Welcome => view! {
                                <div class="studio-welcome">
                                    <h2>"Build something"</h2>
                                    <p>"Select a cartridge to preview, or ask the AI to create one."</p>
                                    <div class="studio-suggestions">
                                        <code>"\"make a snake game\""</code>
                                        <code>"\"build a todo list\""</code>
                                        <code>"\"create a calculator\""</code>
                                    </div>
                                </div>
                            }.into_view(),
                            CenterView::AppPreview(ref slug) => {
                                let url = format!("/app/{slug}");
                                view! {
                                    <div class="studio-preview">
                                        <div class="studio-preview-bar">
                                            <span class="studio-preview-url">{&url}</span>
                                            <a href={url.clone()} target="_blank" class="studio-preview-open">"Open \u{2197}"</a>
                                        </div>
                                        <iframe
                                            src={url}
                                            class="studio-preview-frame"
                                            sandbox="allow-scripts allow-same-origin"
                                        />
                                    </div>
                                }.into_view()
                            },
                            CenterView::CartridgePreview(ref slug) => {
                                let slug_run = slug.clone();
                                let (cartridge_html, set_cartridge_html) = create_signal(String::from("<div class='studio-loading'>Loading...</div>"));
                                let (cartridge_logs, _set_cartridge_logs) = create_signal(Vec::<String>::new());
                                // Fetch the cartridge output from the SERVER (not client-side instantiation).
                                // Backend cartridges are wasip1 binaries that run in wasmtime on the server.
                                spawn_local(async move {
                                    match gloo_net::http::Request::get(&format!("/c/{}", slug_run))
                                        .send()
                                        .await
                                    {
                                        Ok(resp) => {
                                            let ct = resp.headers().get("content-type").unwrap_or_default();
                                            match resp.text().await {
                                                Ok(body) if ct.contains("html") => set_cartridge_html.set(body),
                                                Ok(body) => set_cartridge_html.set(format!("<pre>{body}</pre>")),
                                                Err(e) => set_cartridge_html.set(format!("<pre class='error'>Read error: {e}</pre>")),
                                            }
                                        }
                                        Err(e) => set_cartridge_html.set(format!("<pre class='error'>Fetch error: {e}</pre>")),
                                    }
                                });
                                view! {
                                    <div class="studio-preview">
                                        <div class="studio-preview-bar">
                                            <span class="studio-preview-url">"/c/"{slug}" (WASM)"</span>
                                            <a href={format!("/c/{slug}")} target="_blank" class="studio-preview-open">"Open \u{2197}"</a>
                                        </div>
                                        <div class="studio-cartridge-output" inner_html=move || cartridge_html.get() />
                                        {move || {
                                            let logs = cartridge_logs.get();
                                            (!logs.is_empty()).then(|| view! {
                                                <div class="studio-cartridge-logs">
                                                    {logs.iter().map(|l| view! { <div class="studio-log-line">{l}</div> }).collect_view()}
                                                </div>
                                            })
                                        }}
                                    </div>
                                }.into_view()
                            },
                            CenterView::InteractivePreview(ref slug) => {
                                let slug_run = slug.clone();
                                let (error_msg, set_error) = create_signal(Option::<String>::None);
                                let canvas_ref = create_node_ref::<leptos::html::Canvas>();
                                let raf_id = Rc::new(RefCell::new(0i32));

                                let slug_for_init = slug.clone();
                                let raf_id_clone = raf_id.clone();
                                create_effect(move |_| {
                                    let canvas_el = canvas_ref.get();
                                    if canvas_el.is_none() { return; }
                                    let canvas = canvas_el.unwrap();
                                    let slug = slug_for_init.clone();
                                    let set_err = set_error;
                                    let raf_id = raf_id_clone.clone();

                                    spawn_local(async move {
                                        let bytes = match crate::cartridge_runner::detect_type(&slug).await {
                                            Ok((_, b)) => b,
                                            Err(e) => { set_err.set(Some(e)); return; }
                                        };

                                        let width = 320u32;
                                        let height = 240u32;
                                        canvas.set_width(width);
                                        canvas.set_height(height);
                                        let _ = canvas.focus();

                                        let cart = match crate::cartridge_runner::instantiate_interactive(&bytes, width, height).await {
                                            Ok(c) => c,
                                            Err(e) => { set_err.set(Some(e)); return; }
                                        };

                                        let ctx: web_sys::CanvasRenderingContext2d = canvas
                                            .get_context("2d").unwrap().unwrap().dyn_into().unwrap();

                                        let cart = Rc::new(cart);
                                        let cart_for_loop = cart.clone();
                                        let cart_for_kd = cart.clone();
                                        let cart_for_ku = cart.clone();

                                        let kd = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |ev: web_sys::KeyboardEvent| {
                                            ev.prevent_default();
                                            if let Some(ref f) = cart_for_kd.key_down_fn {
                                                let _ = f.call1(&JsValue::undefined(), &JsValue::from(ev.key_code() as i32));
                                            }
                                        });
                                        let ku = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |ev: web_sys::KeyboardEvent| {
                                            if let Some(ref f) = cart_for_ku.key_up_fn {
                                                let _ = f.call1(&JsValue::undefined(), &JsValue::from(ev.key_code() as i32));
                                            }
                                        });
                                        let _ = canvas.add_event_listener_with_callback("keydown", kd.as_ref().unchecked_ref());
                                        let _ = canvas.add_event_listener_with_callback("keyup", ku.as_ref().unchecked_ref());
                                        kd.forget();
                                        ku.forget();

                                        let window = web_sys::window().unwrap();
                                        let f: Rc<RefCell<Option<wasm_bindgen::closure::Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
                                        let g = f.clone();
                                        let raf_id_inner = raf_id.clone();

                                        *g.borrow_mut() = Some(wasm_bindgen::closure::Closure::new(move || {
                                            let _ = cart_for_loop.tick_fn.call0(&JsValue::undefined());
                                            let pixels = crate::cartridge_runner::read_framebuffer(&cart_for_loop);
                                            if let Ok(img_data) = web_sys::ImageData::new_with_u8_clamped_array_and_sh(
                                                wasm_bindgen::Clamped(pixels.as_slice()),
                                                cart_for_loop.width,
                                                cart_for_loop.height,
                                            ) {
                                                let _ = ctx.put_image_data(&img_data, 0.0, 0.0);
                                            }
                                            let win = web_sys::window().unwrap();
                                            let id = win.request_animation_frame(
                                                f.borrow().as_ref().unwrap().as_ref().unchecked_ref()
                                            ).unwrap_or(0);
                                            *raf_id_inner.borrow_mut() = id;
                                        }));

                                        let id = window.request_animation_frame(
                                            g.borrow().as_ref().unwrap().as_ref().unchecked_ref()
                                        ).unwrap_or(0);
                                        *raf_id.borrow_mut() = id;
                                    });
                                });

                                // Cancel animation on cleanup
                                let raf_for_cleanup = raf_id;
                                on_cleanup(move || {
                                    if let Some(window) = web_sys::window() {
                                        let _ = window.cancel_animation_frame(*raf_for_cleanup.borrow());
                                    }
                                });

                                view! {
                                    <div class="studio-preview">
                                        <div class="studio-preview-bar">
                                            <span class="studio-preview-url">"/c/"{slug_run}" (Interactive WASM)"</span>
                                        </div>
                                        {move || error_msg.get().map(|e| view! {
                                            <div class="studio-error"><pre>{e}</pre></div>
                                        })}
                                        <div class="studio-canvas-container">
                                            <canvas
                                                node_ref=canvas_ref
                                                class="studio-canvas"
                                                tabindex="0"
                                                width="320"
                                                height="240"
                                            />
                                        </div>
                                    </div>
                                }.into_view()
                            },
                            CenterView::FrontendPreview(ref slug) => {
                                let slug_display = slug.clone();
                                let mount_id = format!("cartridge-mount-{slug}");
                                let mount_id_for_load = mount_id.clone();
                                let slug_for_load = slug.clone();
                                let (load_error, set_load_error) = create_signal(Option::<String>::None);
                                let (loading, set_loading) = create_signal(true);

                                // Load the frontend cartridge on mount
                                spawn_local(async move {
                                    match crate::cartridge_runner::load_frontend_cartridge(&slug_for_load, &mount_id_for_load).await {
                                        Ok(()) => set_loading.set(false),
                                        Err(e) => {
                                            set_loading.set(false);
                                            set_load_error.set(Some(e));
                                        }
                                    }
                                });

                                view! {
                                    <div class="studio-preview studio-frontend-preview">
                                        <div class="studio-preview-bar">
                                            <span class="studio-preview-url">"/c/"{slug_display}" (Frontend Leptos App)"</span>
                                        </div>
                                        {move || loading.get().then(|| view! {
                                            <div class="studio-loading">"Loading cartridge..."</div>
                                        })}
                                        {move || load_error.get().map(|e| view! {
                                            <div class="studio-error"><pre>{e}</pre></div>
                                        })}
                                        <div id={mount_id} class="studio-cartridge-mount"></div>
                                    </div>
                                }.into_view()
                            },
                            CenterView::FileView(path, content) => view! {
                                <div class="studio-editor">
                                    <div class="studio-editor-bar">{path}</div>
                                    <pre class="studio-code"><code>{content}</code></pre>
                                </div>
                            }.into_view(),
                        }
                    }}
                </div>

                // ── Right: Chat ──
                <div class="studio-chat">
                    <div class="studio-chat-messages" node_ref=messages_ref>
                        {move || {
                            let msgs = messages.get();
                            if msgs.is_empty() {
                                view! {
                                    <div class="studio-chat-empty">
                                        <p>"Start a conversation"</p>
                                        <p class="studio-hint">"Tell the AI what to build"</p>
                                    </div>
                                }.into_view()
                            } else {
                                msgs.iter().map(|msg| {
                                    let is_user = msg.role == "user";
                                    let content = msg.content.clone();
                                    let tools = msg.tools.clone();
                                    view! {
                                        <div class=if is_user { "studio-msg studio-msg--user" } else { "studio-msg studio-msg--ai" }>
                                            <div class="studio-msg-role">{if is_user { "You" } else { "Soul" }}</div>
                                            <div class="studio-msg-content">{content}</div>
                                            // Tool executions
                                            {(!tools.is_empty()).then(|| {
                                                view! {
                                                    <div class="studio-msg-tools">
                                                        {tools.iter().map(|t| {
                                                            let cmd = t.get("command").and_then(|v| v.as_str()).unwrap_or("?").to_string();
                                                            let stdout = t.get("stdout").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                            let stderr = t.get("stderr").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                            let exit_code = t.get("exit_code").and_then(|v| v.as_i64()).unwrap_or(-1);
                                                            let (expanded, set_expanded) = create_signal(false);
                                                            let output = if !stderr.is_empty() {
                                                                format!("{stdout}\n{stderr}")
                                                            } else {
                                                                stdout.clone()
                                                            };
                                                            let output_for_view = output.clone();
                                                            view! {
                                                                <div class="chat-tool-block">
                                                                    <button class="chat-tool-header" on:click=move |_| set_expanded.update(|v| *v = !*v)>
                                                                        <span class="chat-tool-cmd">"$ "{&cmd}</span>
                                                                        <span class="chat-tool-exit">{format!("exit {exit_code}")}</span>
                                                                    </button>
                                                                    {move || expanded.get().then(|| {
                                                                        let o = output_for_view.clone();
                                                                        view! { <pre class="chat-tool-output">{o}</pre> }
                                                                    })}
                                                                </div>
                                                            }
                                                        }).collect_view()}
                                                    </div>
                                                }
                                            })}
                                            // Feedback buttons (AI messages only)
                                            {(!is_user).then(|| {
                                                let (feedback_given, set_feedback_given) = create_signal(Option::<String>::None);
                                                view! {
                                                    <div class="studio-msg-feedback">
                                                        {move || {
                                                            match feedback_given.get() {
                                                                Some(ref fb) => view! {
                                                                    <span class="studio-feedback-done">{fb.clone()}</span>
                                                                }.into_view(),
                                                                None => view! {
                                                                    <button class="studio-feedback-btn" on:click=move |_| {
                                                                        set_feedback_given.set(Some("good".into()));
                                                                        spawn_local(async move {
                                                                            let _ = gloo_net::http::Request::post("/soul/admin/reward")
                                                                                .json(&serde_json::json!({"commit_sha": "chat-feedback"})).unwrap()
                                                                                .send().await;
                                                                        });
                                                                    } title="Good">"+"</button>
                                                                    <button class="studio-feedback-btn" on:click=move |_| {
                                                                        set_feedback_given.set(Some("bad".into()));
                                                                        spawn_local(async move {
                                                                            let _ = gloo_net::http::Request::post("/soul/admin/penalty")
                                                                                .json(&serde_json::json!({"commit_sha": "chat-feedback"})).unwrap()
                                                                                .send().await;
                                                                        });
                                                                    } title="Bad">"-"</button>
                                                                }.into_view(),
                                                            }
                                                        }}
                                                    </div>
                                                }
                                            })}
                                        </div>
                                    }
                                }).collect_view().into_view()
                            }
                        }}
                        {move || sending.get().then(|| view! {
                            <div class="studio-msg studio-msg--ai studio-typing">"Thinking..."</div>
                        })}
                    </div>
                    <div class="studio-chat-input">
                        <textarea
                            placeholder="Tell the soul what to build..."
                            prop:value=move || input.get()
                            on:input=move |ev| set_input.set(event_target_value(&ev))
                            on:keydown=on_keydown
                            rows="2"
                        />
                        <button
                            class="btn btn-primary btn-sm"
                            on:click=move |_| send_message()
                            disabled=move || sending.get()
                        >"Send"</button>
                    </div>
                </div>
            </div>

            // ── Status bar ──
            <div class="studio-statusbar">
                {move || {
                    let s = soul_status.get();
                    let fitness = s.as_ref().and_then(|d| d.get("fitness")).and_then(|f| f.get("total")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let fe = s.as_ref().and_then(|d| d.get("free_energy")).and_then(|f| f.get("F")).and_then(|v| v.as_str()).unwrap_or("--").to_string();
                    let regime = s.as_ref().and_then(|d| d.get("free_energy")).and_then(|f| f.get("regime")).and_then(|v| v.as_str()).unwrap_or("--").to_string();
                    let elo = s.as_ref().and_then(|d| d.get("benchmark")).and_then(|b| b.get("elo")).and_then(|v| v.as_str()).unwrap_or("--").to_string();
                    let psi = s.as_ref().and_then(|d| d.get("colony")).and_then(|c| c.get("psi")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let psi_trend = s.as_ref().and_then(|d| d.get("colony")).and_then(|c| c.get("psi_trend")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let psi_arrow = if psi_trend > 0.001 { "\u{2191}" } else if psi_trend < -0.001 { "\u{2193}" } else { "\u{2192}" };
                    let m = sys_metrics.get();
                    let cpu = m.as_ref().and_then(|d| d.get("cpu_pct")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let mem_pct = m.as_ref().and_then(|d| d.get("mem_pct")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let disk_pct = m.as_ref().and_then(|d| d.get("disk_pct")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let disk_class = if disk_pct > 80.0 { "studio-metric-warn" } else { "" };
                    view! {
                        <span>{format!("Fitness {:.0}%", fitness * 100.0)}</span>
                        <span class="studio-statusbar-sep">"|"</span>
                        <span>{format!("F={fe}")}</span>
                        <span class="studio-statusbar-badge">{regime}</span>
                        <span class="studio-statusbar-sep">"|"</span>
                        <span>{format!("ELO {elo}")}</span>
                        <span class="studio-statusbar-sep">"|"</span>
                        <span>{format!("\u{03A8}={psi:.2}{psi_arrow}")}</span>
                        <span class="studio-statusbar-sep">"|"</span>
                        <span>{format!("CPU {cpu:.0}%")}</span>
                        <span>{format!("RAM {mem_pct:.0}%")}</span>
                        <span class={disk_class}>{format!("Disk {disk_pct:.0}%")}</span>
                    }
                }}
            </div>
        </div>
    }
}

/// Fetch file tree from the admin ls endpoint.
async fn fetch_file_tree(path: &str) -> Result<Vec<FileEntry>, String> {
    let resp = gloo_net::http::Request::get(&format!("/soul/admin/ls?path={}", path))
        .send()
        .await
        .map_err(|e| format!("Failed: {e}"))?;

    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }

    resp.json::<Vec<FileEntry>>()
        .await
        .map_err(|e| format!("Parse error: {e}"))
}

/// Fetch file content from the admin cat endpoint.
async fn fetch_file_content(path: &str) -> Result<String, String> {
    let resp = gloo_net::http::Request::get(&format!("/soul/admin/cat?path={}", path))
        .send()
        .await
        .map_err(|e| format!("Failed: {e}"))?;

    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }

    resp.text().await.map_err(|e| format!("Read error: {e}"))
}
