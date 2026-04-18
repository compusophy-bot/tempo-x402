use leptos::*;
use wasm_bindgen::prelude::*;

#[component]
fn App() -> impl IntoView {
    let (height_cm, set_height_cm) = create_signal("170".to_string());
    let (weight_kg, set_weight_kg) = create_signal("70".to_string());
    let (use_imperial, set_use_imperial) = create_signal(false);

    let bmi = move || {
        let h: f64 = height_cm.get().parse().unwrap_or(0.0);
        let w: f64 = weight_kg.get().parse().unwrap_or(0.0);
        if h <= 0.0 || w <= 0.0 { return 0.0; }

        if use_imperial.get() {
            // height in inches, weight in pounds
            (w / (h * h)) * 703.0
        } else {
            // height in cm, weight in kg
            let h_m = h / 100.0;
            w / (h_m * h_m)
        }
    };

    let category = move || {
        let b = bmi();
        if b <= 0.0 { ("Enter values", "#666") }
        else if b < 18.5 { ("Underweight", "#61afef") }
        else if b < 25.0 { ("Normal weight", "#98c379") }
        else if b < 30.0 { ("Overweight", "#e5c07b") }
        else if b < 35.0 { ("Obese (Class I)", "#d19a66") }
        else if b < 40.0 { ("Obese (Class II)", "#e06c75") }
        else { ("Obese (Class III)", "#be5046") }
    };

    let bar_width = move || {
        let b = bmi();
        let pct = ((b / 50.0) * 100.0).min(100.0).max(0.0);
        format!("{}%", pct)
    };

    let input_style = "width: 100%; padding: 12px; background: #111; border: 1px solid #333; \
                       color: #e0e0e0; border-radius: 6px; font-size: 18px; outline: none; \
                       text-align: center;";
    let label_style = "font-size: 13px; color: #888; text-transform: uppercase; letter-spacing: 1px;";

    view! {
        <div style="font-family: 'Segoe UI', sans-serif; background: #0a0a0a; color: #e0e0e0; \
                     min-height: 100vh; display: flex; flex-direction: column; align-items: center; \
                     justify-content: center; gap: 24px; padding: 40px 20px;">
            <h1 style="font-size: 28px; color: #7fdbca; margin: 0;">"BMI Calculator"</h1>

            <button
                style="padding: 8px 20px; background: #1a1a2e; border: 1px solid #333; \
                       color: #7fdbca; cursor: pointer; border-radius: 6px; font-size: 13px;"
                on:click=move |_| {
                    let imperial = !use_imperial.get();
                    set_use_imperial.set(imperial);
                    if imperial {
                        // Convert current values to imperial
                        let h: f64 = height_cm.get().parse().unwrap_or(170.0);
                        let w: f64 = weight_kg.get().parse().unwrap_or(70.0);
                        set_height_cm.set(format!("{:.0}", h / 2.54));
                        set_weight_kg.set(format!("{:.0}", w * 2.205));
                    } else {
                        let h: f64 = height_cm.get().parse().unwrap_or(67.0);
                        let w: f64 = weight_kg.get().parse().unwrap_or(154.0);
                        set_height_cm.set(format!("{:.0}", h * 2.54));
                        set_weight_kg.set(format!("{:.0}", w / 2.205));
                    }
                }
            >
                {move || if use_imperial.get() { "Switch to Metric" } else { "Switch to Imperial" }}
            </button>

            <div style="width: 100%; max-width: 320px; display: flex; flex-direction: column; \
                        gap: 16px;">
                <div style="display: flex; flex-direction: column; gap: 6px;">
                    <label style=label_style>
                        {move || if use_imperial.get() { "Height (inches)" } else { "Height (cm)" }}
                    </label>
                    <input
                        type="number"
                        step="1"
                        style=input_style
                        prop:value=height_cm
                        on:input=move |ev| set_height_cm.set(event_target_value(&ev))
                    />
                </div>
                <div style="display: flex; flex-direction: column; gap: 6px;">
                    <label style=label_style>
                        {move || if use_imperial.get() { "Weight (lbs)" } else { "Weight (kg)" }}
                    </label>
                    <input
                        type="number"
                        step="0.1"
                        style=input_style
                        prop:value=weight_kg
                        on:input=move |ev| set_weight_kg.set(event_target_value(&ev))
                    />
                </div>
            </div>

            <div style="text-align: center;">
                <div style="font-size: 56px; font-weight: bold; font-variant-numeric: tabular-nums;">
                    <span style=move || format!("color: {};", category().1)>
                        {move || if bmi() > 0.0 { format!("{:.1}", bmi()) } else { "--".to_string() }}
                    </span>
                </div>
                <div style=move || format!("font-size: 18px; color: {}; margin-top: 4px;", category().1)>
                    {move || category().0}
                </div>
            </div>

            <div style="width: 100%; max-width: 320px; height: 12px; background: #111; \
                        border-radius: 6px; overflow: hidden; border: 1px solid #222;">
                <div style=move || format!(
                    "height: 100%; width: {}; background: {}; border-radius: 6px; \
                     transition: width 0.3s, background 0.3s;",
                    bar_width(), category().1
                )></div>
            </div>

            <div style="display: flex; gap: 16px; color: #555; font-size: 11px;">
                <span>"<18.5 Under"</span>
                <span>"18.5-25 Normal"</span>
                <span>"25-30 Over"</span>
                <span>">30 Obese"</span>
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
