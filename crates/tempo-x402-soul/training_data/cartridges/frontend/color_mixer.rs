use leptos::*;
use wasm_bindgen::prelude::*;

#[component]
fn App() -> impl IntoView {
    let (red, set_red) = create_signal(128u8);
    let (green, set_green) = create_signal(64u8);
    let (blue, set_blue) = create_signal(200u8);

    let hex = move || format!("#{:02X}{:02X}{:02X}", red.get(), green.get(), blue.get());
    let rgb_str = move || format!("rgb({}, {}, {})", red.get(), green.get(), blue.get());

    let (copied, set_copied) = create_signal(false);

    let copy_hex = move |_| {
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        let el = document.create_element("textarea").unwrap();
        el.set_text_content(Some(&hex()));
        let body = document.body().unwrap();
        let _ = body.append_child(&el);
        let _ = body.remove_child(&el);
        set_copied.set(true);
    };

    let slider_track = "width: 100%; height: 8px; appearance: none; -webkit-appearance: none; \
                        border-radius: 4px; outline: none; cursor: pointer;";

    let slider_row = move |label: &'static str, color: &'static str, value: Signal<u8>,
                            setter: WriteSignal<u8>| {
        view! {
            <div style="display: flex; align-items: center; gap: 12px; width: 100%;">
                <span style=format!("color: {}; font-weight: bold; width: 16px;", color)>{label}</span>
                <input
                    type="range"
                    min="0"
                    max="255"
                    prop:value=move || value.get().to_string()
                    on:input=move |ev| {
                        let v: u8 = event_target_value(&ev).parse().unwrap_or(0);
                        setter.set(v);
                        set_copied.set(false);
                    }
                    style=slider_track
                />
                <span style="color: #888; font-variant-numeric: tabular-nums; width: 30px; \
                             text-align: right; font-size: 14px;">
                    {move || value.get().to_string()}
                </span>
            </div>
        }
    };

    let (r_sig, _) = create_signal(());
    let _ = r_sig;
    let red_signal: Signal<u8> = red.into();
    let green_signal: Signal<u8> = green.into();
    let blue_signal: Signal<u8> = blue.into();

    view! {
        <div style="font-family: 'Segoe UI', sans-serif; background: #0a0a0a; color: #e0e0e0; \
                     min-height: 100vh; display: flex; flex-direction: column; align-items: center; \
                     justify-content: center; gap: 24px; padding: 40px 20px;">
            <h1 style="font-size: 28px; color: #7fdbca; margin: 0;">"Color Mixer"</h1>

            <div style=move || format!(
                "width: 200px; height: 200px; border-radius: 16px; border: 2px solid #333; \
                 background: {};", hex()
            )></div>

            <div style="display: flex; flex-direction: column; gap: 16px; width: 100%; \
                        max-width: 360px;">
                {slider_row("R", "#e06c75", red_signal, set_red)}
                {slider_row("G", "#98c379", green_signal, set_green)}
                {slider_row("B", "#61afef", blue_signal, set_blue)}
            </div>

            <div style="display: flex; flex-direction: column; align-items: center; gap: 8px;">
                <div style="font-size: 24px; font-weight: bold; font-family: monospace;">
                    {hex}
                </div>
                <div style="font-size: 14px; color: #888; font-family: monospace;">
                    {rgb_str}
                </div>
                <button
                    style="padding: 8px 20px; background: #1a1a2e; border: 1px solid #333; \
                           color: #7fdbca; cursor: pointer; border-radius: 6px; font-size: 13px; \
                           margin-top: 4px;"
                    on:click=copy_hex
                >
                    {move || if copied.get() { "Copied!" } else { "Copy Hex" }}
                </button>
            </div>
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
