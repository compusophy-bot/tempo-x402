use leptos::*;
use wasm_bindgen::prelude::*;

#[component]
fn App() -> impl IntoView {
    let (count, set_count) = create_signal(0i64);

    let decrement = move |_| set_count.update(|c| *c -= 1);
    let increment = move |_| set_count.update(|c| *c += 1);
    let reset = move |_| set_count.set(0);

    let btn_style = "padding: 12px 24px; font-size: 18px; border: 1px solid #333; \
                     background: #1a1a2e; color: #e0e0e0; cursor: pointer; border-radius: 6px; \
                     min-width: 50px; transition: background 0.2s;";

    view! {
        <div style="font-family: 'Segoe UI', sans-serif; background: #0a0a0a; color: #e0e0e0; \
                     min-height: 100vh; display: flex; flex-direction: column; align-items: center; \
                     justify-content: center; gap: 24px;">
            <h1 style="font-size: 32px; color: #7fdbca; margin: 0;">"Counter"</h1>
            <div style="font-size: 72px; font-weight: bold; color: #c792ea; \
                        font-variant-numeric: tabular-nums;">
                {count}
            </div>
            <div style="display: flex; gap: 12px;">
                <button style=btn_style on:click=decrement>"-"</button>
                <button style=btn_style on:click=reset>"Reset"</button>
                <button style=btn_style on:click=increment>"+"</button>
            </div>
            <p style="color: #666; font-size: 14px;">
                {move || if count.get() > 0 { "Positive".to_string() }
                         else if count.get() < 0 { "Negative".to_string() }
                         else { "Zero".to_string() }}
            </p>
        </div>
    }
}

#[wasm_bindgen]
pub fn init(selector: &str) {
    console_error_panic_hook::set_once();
    let document = web_sys::window().unwrap().document().unwrap();
    let el = document.query_selector(selector).unwrap().unwrap();
    mount_to(el.unchecked_into(), App);
}
