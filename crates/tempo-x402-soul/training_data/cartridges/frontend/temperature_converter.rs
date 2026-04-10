use leptos::*;
use wasm_bindgen::prelude::*;

#[component]
fn App() -> impl IntoView {
    let (celsius, set_celsius) = create_signal(0.0f64);
    let (source, set_source) = create_signal("c".to_string());

    let fahrenheit = move || celsius.get() * 9.0 / 5.0 + 32.0;
    let kelvin = move || celsius.get() + 273.15;

    let format_val = |v: f64| -> String {
        if (v - v.round()).abs() < 0.005 {
            format!("{:.0}", v)
        } else {
            format!("{:.2}", v)
        }
    };

    let on_celsius = move |ev: web_sys::Event| {
        let val = event_target_value(&ev);
        if let Ok(v) = val.parse::<f64>() {
            set_celsius.set(v);
            set_source.set("c".to_string());
        }
    };

    let on_fahrenheit = move |ev: web_sys::Event| {
        let val = event_target_value(&ev);
        if let Ok(v) = val.parse::<f64>() {
            set_celsius.set((v - 32.0) * 5.0 / 9.0);
            set_source.set("f".to_string());
        }
    };

    let on_kelvin = move |ev: web_sys::Event| {
        let val = event_target_value(&ev);
        if let Ok(v) = val.parse::<f64>() {
            set_celsius.set(v - 273.15);
            set_source.set("k".to_string());
        }
    };

    let input_style = "width: 100%; padding: 12px 14px; background: #111; border: 1px solid #333; \
                       color: #e0e0e0; border-radius: 6px; font-size: 18px; outline: none; \
                       text-align: center; font-variant-numeric: tabular-nums;";
    let label_style = "font-size: 14px; color: #888; text-transform: uppercase; letter-spacing: 1px;";

    let temp_color = move || {
        let c = celsius.get();
        if c <= 0.0 { "#61afef" }
        else if c <= 20.0 { "#7fdbca" }
        else if c <= 35.0 { "#e5c07b" }
        else { "#e06c75" }
    };

    view! {
        <div style="font-family: 'Segoe UI', sans-serif; background: #0a0a0a; color: #e0e0e0; \
                     min-height: 100vh; display: flex; flex-direction: column; align-items: center; \
                     justify-content: center; gap: 32px; padding: 40px 20px;">
            <h1 style="font-size: 28px; color: #7fdbca; margin: 0;">"Temperature Converter"</h1>

            <div style=move || format!(
                "font-size: 48px; font-weight: bold; color: {};", temp_color()
            )>
                {move || format!("{}\u{00b0}C", format_val(celsius.get()))}
            </div>

            <div style="display: flex; flex-direction: column; gap: 20px; width: 100%; \
                        max-width: 300px;">
                <div style="display: flex; flex-direction: column; gap: 6px;">
                    <label style=label_style>"Celsius (\u{00b0}C)"</label>
                    <input
                        type="number"
                        step="0.1"
                        style=input_style
                        prop:value=move || {
                            if source.get() == "c" { format_val(celsius.get()) }
                            else { format_val(celsius.get()) }
                        }
                        on:input=on_celsius
                    />
                </div>
                <div style="display: flex; flex-direction: column; gap: 6px;">
                    <label style=label_style>"Fahrenheit (\u{00b0}F)"</label>
                    <input
                        type="number"
                        step="0.1"
                        style=input_style
                        prop:value=move || format_val(fahrenheit())
                        on:input=on_fahrenheit
                    />
                </div>
                <div style="display: flex; flex-direction: column; gap: 6px;">
                    <label style=label_style>"Kelvin (K)"</label>
                    <input
                        type="number"
                        step="0.1"
                        style=input_style
                        prop:value=move || format_val(kelvin())
                        on:input=on_kelvin
                    />
                </div>
            </div>

            <div style="color: #555; font-size: 13px; text-align: center;">
                {move || {
                    let c = celsius.get();
                    if c <= 0.0 { "Freezing cold" }
                    else if c <= 10.0 { "Very cold" }
                    else if c <= 20.0 { "Cool" }
                    else if c <= 30.0 { "Warm" }
                    else if c <= 40.0 { "Hot" }
                    else { "Extremely hot" }
                }}
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
