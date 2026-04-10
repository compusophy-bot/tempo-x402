use leptos::*;
use wasm_bindgen::prelude::*;

#[derive(Clone, Copy, PartialEq)]
enum Category {
    Length,
    Weight,
    Temperature,
}

#[component]
fn App() -> impl IntoView {
    let (category, set_category) = create_signal(Category::Length);
    let (value, set_value) = create_signal(1.0f64);
    let (from_unit, set_from_unit) = create_signal(0usize);
    let (to_unit, set_to_unit) = create_signal(1usize);

    let units = move || match category.get() {
        Category::Length => vec!["meters", "kilometers", "miles", "feet", "inches", "centimeters"],
        Category::Weight => vec!["kilograms", "grams", "pounds", "ounces"],
        Category::Temperature => vec!["Celsius", "Fahrenheit", "Kelvin"],
    };

    let length_factors: Vec<f64> = vec![1.0, 1000.0, 1609.344, 0.3048, 0.0254, 0.01];
    let weight_factors: Vec<f64> = vec![1.0, 0.001, 0.453592, 0.0283495];

    let convert = move || {
        let v = value.get();
        let f = from_unit.get();
        let t = to_unit.get();
        match category.get() {
            Category::Length => {
                v * length_factors[f] / length_factors[t]
            }
            Category::Weight => {
                v * weight_factors[f] / weight_factors[t]
            }
            Category::Temperature => {
                let celsius = match f {
                    0 => v,
                    1 => (v - 32.0) * 5.0 / 9.0,
                    2 => v - 273.15,
                    _ => v,
                };
                match t {
                    0 => celsius,
                    1 => celsius * 9.0 / 5.0 + 32.0,
                    2 => celsius + 273.15,
                    _ => celsius,
                }
            }
        }
    };

    let swap = move |_| {
        let f = from_unit.get();
        let t = to_unit.get();
        set_from_unit.set(t);
        set_to_unit.set(f);
    };

    let categories = vec![
        (Category::Length, "Length"),
        (Category::Weight, "Weight"),
        (Category::Temperature, "Temperature"),
    ];

    view! {
        <div style="font-family: 'Segoe UI', sans-serif; background: #0f0f23; color: #ccc; \
                     min-height: 100vh; padding: 40px 20px; display: flex; flex-direction: column; \
                     align-items: center;">
            <h1 style="color: #00cc7a; margin-bottom: 24px;">"Unit Converter"</h1>

            <div style="display: flex; gap: 8px; margin-bottom: 20px;">
                {categories.into_iter().map(|(cat, label)| {
                    view! {
                        <button
                            on:click=move |_| {
                                set_category.set(cat);
                                set_from_unit.set(0);
                                set_to_unit.set(1);
                            }
                            style=move || format!(
                                "padding: 8px 16px; border-radius: 6px; cursor: pointer; font-size: 14px; \
                                 border: 1px solid {}; background: {}; color: {};",
                                if category.get() == cat { "#00cc7a" } else { "#333" },
                                if category.get() == cat { "#0a2a1a" } else { "#1a1a3e" },
                                if category.get() == cat { "#00cc7a" } else { "#888" }
                            )
                        >
                            {label}
                        </button>
                    }
                }).collect::<Vec<_>>()}
            </div>

            <div style="max-width: 400px; width: 100%; background: #1a1a2e; padding: 24px; \
                        border-radius: 12px;">
                <input type="number" step="any"
                    prop:value=move || format!("{}", value.get())
                    on:input=move |ev| {
                        if let Ok(v) = event_target_value(&ev).parse::<f64>() {
                            set_value.set(v);
                        }
                    }
                    style="width: 100%; padding: 14px; background: #111; border: 1px solid #333; \
                           color: #e0e0e0; border-radius: 8px; font-size: 20px; text-align: center; \
                           margin-bottom: 12px;"
                />

                <div style="display: flex; gap: 8px; align-items: center; margin-bottom: 12px;">
                    <select
                        on:change=move |ev| {
                            if let Ok(v) = event_target_value(&ev).parse::<usize>() {
                                set_from_unit.set(v);
                            }
                        }
                        prop:value=move || from_unit.get().to_string()
                        style="flex: 1; padding: 10px; background: #111; border: 1px solid #333; \
                               color: #ccc; border-radius: 6px; font-size: 14px;"
                    >
                        {move || units().into_iter().enumerate().map(|(i, u)| {
                            view! { <option value=i.to_string() selected=move || from_unit.get() == i>{u}</option> }
                        }).collect::<Vec<_>>()}
                    </select>

                    <button
                        on:click=swap
                        style="padding: 8px 12px; background: none; border: 1px solid #00cc7a; \
                               color: #00cc7a; border-radius: 6px; cursor: pointer; font-size: 18px;"
                    >"\u{21C6}"</button>

                    <select
                        on:change=move |ev| {
                            if let Ok(v) = event_target_value(&ev).parse::<usize>() {
                                set_to_unit.set(v);
                            }
                        }
                        prop:value=move || to_unit.get().to_string()
                        style="flex: 1; padding: 10px; background: #111; border: 1px solid #333; \
                               color: #ccc; border-radius: 6px; font-size: 14px;"
                    >
                        {move || units().into_iter().enumerate().map(|(i, u)| {
                            view! { <option value=i.to_string() selected=move || to_unit.get() == i>{u}</option> }
                        }).collect::<Vec<_>>()}
                    </select>
                </div>

                <div style="text-align: center; font-size: 28px; color: #00cc7a; padding: 16px; \
                            background: #0a2a1a; border-radius: 8px;">
                    {move || {
                        let result = convert();
                        if result.abs() < 0.001 && result != 0.0 {
                            format!("{:.6}", result)
                        } else {
                            format!("{:.4}", result)
                        }
                    }}
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
