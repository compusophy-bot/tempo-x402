use leptos::*;
use wasm_bindgen::prelude::*;

#[component]
fn App() -> impl IntoView {
    let (count, set_count) = create_signal(0);
    view! {
        <div>
            <h1>"Hello from Leptos"</h1>
            <button on:click=move |_| set_count.update(|c| *c += 1)>
                "Count: " {count}
            </button>
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
