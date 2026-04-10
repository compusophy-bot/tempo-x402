use leptos::*;
use wasm_bindgen::prelude::*;

#[component]
fn App() -> impl IntoView {
    let (bill, set_bill) = create_signal(50.0f64);
    let (tip_pct, set_tip_pct) = create_signal(15u32);
    let (split, set_split) = create_signal(1u32);

    let tip_amount = move || bill.get() * tip_pct.get() as f64 / 100.0;
    let total = move || bill.get() + tip_amount();
    let per_person = move || total() / split.get().max(1) as f64;

    let presets = vec![10u32, 15, 18, 20, 25];

    view! {
        <div style="font-family: 'Segoe UI', sans-serif; background: #0f172a; color: #e2e8f0; \
                     min-height: 100vh; padding: 40px 20px; display: flex; flex-direction: column; \
                     align-items: center;">
            <h1 style="color: #38bdf8; margin-bottom: 24px;">"Tip Calculator"</h1>

            <div style="max-width: 400px; width: 100%; background: #1e293b; padding: 24px; \
                        border-radius: 16px;">
                <label style="display: block; font-size: 13px; color: #94a3b8; text-transform: uppercase; \
                              margin-bottom: 4px;">"Bill Amount ($)"</label>
                <input type="number" step="0.01" min="0"
                    prop:value=move || format!("{:.2}", bill.get())
                    on:input=move |ev| {
                        if let Ok(v) = event_target_value(&ev).parse::<f64>() {
                            set_bill.set(v);
                        }
                    }
                    style="width: 100%; padding: 14px; background: #0f172a; border: 2px solid #334155; \
                           color: #e2e8f0; border-radius: 8px; font-size: 24px; text-align: center; \
                           margin-bottom: 16px;"
                />

                <label style="display: block; font-size: 13px; color: #94a3b8; text-transform: uppercase; \
                              margin-bottom: 8px;">"Tip Percentage"</label>
                <div style="display: flex; gap: 8px; margin-bottom: 16px;">
                    {presets.into_iter().map(|p| {
                        view! {
                            <button
                                on:click=move |_| set_tip_pct.set(p)
                                style=move || format!(
                                    "flex: 1; padding: 12px; border-radius: 8px; cursor: pointer; \
                                     font-size: 16px; font-weight: 600; border: 2px solid {}; \
                                     background: {}; color: #e2e8f0;",
                                    if tip_pct.get() == p { "#38bdf8" } else { "transparent" },
                                    if tip_pct.get() == p { "#0c4a6e" } else { "#334155" }
                                )
                            >
                                {format!("{}%", p)}
                            </button>
                        }
                    }).collect::<Vec<_>>()}
                </div>

                <label style="display: block; font-size: 13px; color: #94a3b8; text-transform: uppercase; \
                              margin-bottom: 8px;">"Split Between"</label>
                <div style="display: flex; align-items: center; gap: 12px; margin-bottom: 20px;">
                    <button
                        on:click=move |_| set_split.update(|s| if *s > 1 { *s -= 1 })
                        style="width: 44px; height: 44px; border-radius: 50%; background: #334155; \
                               border: none; color: #e2e8f0; font-size: 20px; cursor: pointer;"
                    >"-"</button>
                    <span style="font-size: 24px; font-weight: bold; min-width: 40px; text-align: center;">
                        {split}
                    </span>
                    <button
                        on:click=move |_| set_split.update(|s| *s += 1)
                        style="width: 44px; height: 44px; border-radius: 50%; background: #334155; \
                               border: none; color: #e2e8f0; font-size: 20px; cursor: pointer;"
                    >"+"</button>
                </div>

                <div style="background: #0f172a; padding: 20px; border-radius: 12px;">
                    <div style="display: flex; justify-content: space-between; padding: 8px 0; font-size: 16px;">
                        <span>"Tip"</span>
                        <span>{move || format!("${:.2}", tip_amount())}</span>
                    </div>
                    <div style="display: flex; justify-content: space-between; padding: 8px 0; font-size: 16px;">
                        <span>"Total"</span>
                        <span>{move || format!("${:.2}", total())}</span>
                    </div>
                    <div style="display: flex; justify-content: space-between; padding: 12px 0; \
                                font-size: 22px; font-weight: bold; color: #38bdf8; \
                                border-top: 2px solid #334155; margin-top: 8px;">
                        <span>"Per Person"</span>
                        <span>{move || format!("${:.2}", per_person())}</span>
                    </div>
                </div>
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
