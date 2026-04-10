use leptos::*;
use wasm_bindgen::prelude::*;

#[component]
fn App() -> impl IntoView {
    let (count, set_count) = create_signal(2u32);
    let (sides, set_sides) = create_signal(6u32);
    let (results, set_results) = create_signal(Vec::<u32>::new());
    let (total, set_total) = create_signal(0u32);
    let (seed, set_seed) = create_signal(12345u32);
    let (history, set_history) = create_signal(Vec::<String>::new());

    let roll = move |_| {
        let c = count.get();
        let s = sides.get();
        let mut cur_seed = seed.get();
        let mut rolls = Vec::new();
        let mut sum = 0u32;
        for _ in 0..c {
            cur_seed = cur_seed.wrapping_mul(1103515245).wrapping_add(12345);
            let val = ((cur_seed >> 16) % s) + 1;
            sum += val;
            rolls.push(val);
        }
        set_seed.set(cur_seed);
        set_results.set(rolls);
        set_total.set(sum);
        set_history.update(|h| {
            h.insert(0, format!("{}d{} = {}", c, s, sum));
            if h.len() > 15 { h.truncate(15); }
        });
    };

    let results_list = move || results.get().into_iter().enumerate().collect::<Vec<_>>();
    let history_list = move || history.get().into_iter().enumerate().collect::<Vec<_>>();

    view! {
        <div style="font-family: 'Segoe UI', sans-serif; background: #1a0a2e; color: #e0e0e0; \
                     min-height: 100vh; padding: 40px 20px; display: flex; flex-direction: column; \
                     align-items: center;">
            <h1 style="color: #ff6b6b; margin-bottom: 20px;">"Dice Roller"</h1>

            <div style="display: flex; gap: 12px; align-items: center; margin-bottom: 24px;">
                <label>"Count:"</label>
                <input type="number" min="1" max="10"
                    prop:value=move || count.get().to_string()
                    on:input=move |ev| {
                        if let Ok(v) = event_target_value(&ev).parse::<u32>() {
                            set_count.set(v.max(1).min(10));
                        }
                    }
                    style="width: 60px; padding: 8px; background: #0a0a1a; border: 1px solid #333; \
                           color: #e0e0e0; border-radius: 6px; text-align: center;"
                />
                <label>"Sides:"</label>
                <select
                    on:change=move |ev| {
                        if let Ok(v) = event_target_value(&ev).parse::<u32>() {
                            set_sides.set(v);
                        }
                    }
                    style="padding: 8px; background: #0a0a1a; border: 1px solid #333; \
                           color: #e0e0e0; border-radius: 6px;"
                >
                    <option value="4">"d4"</option>
                    <option value="6" selected>"d6"</option>
                    <option value="8">"d8"</option>
                    <option value="10">"d10"</option>
                    <option value="12">"d12"</option>
                    <option value="20">"d20"</option>
                    <option value="100">"d100"</option>
                </select>
            </div>

            <button
                on:click=roll
                style="padding: 16px 48px; background: #ff6b6b; color: #fff; border: none; \
                       border-radius: 12px; cursor: pointer; font-size: 20px; font-weight: bold; \
                       margin-bottom: 24px;"
            >"ROLL!"</button>

            <div style="display: flex; gap: 12px; flex-wrap: wrap; justify-content: center; \
                        margin-bottom: 16px; min-height: 80px;">
                <For
                    each=results_list
                    key=|(i, _)| *i
                    children=move |(_, val)| {
                        view! {
                            <div style="width: 64px; height: 64px; background: #2d1b69; \
                                        border: 2px solid #7c4dff; border-radius: 10px; \
                                        display: flex; align-items: center; justify-content: center; \
                                        font-size: 28px; font-weight: bold; color: #ff6b6b;">
                                {val}
                            </div>
                        }
                    }
                />
            </div>

            <div style="font-size: 36px; color: #ffd700; font-weight: bold; margin-bottom: 24px;">
                {move || if total.get() > 0 { format!("Total: {}", total.get()) } else { String::new() }}
            </div>

            <div style="width: 100%; max-width: 300px;">
                <h3 style="color: #888; font-size: 13px; margin-bottom: 8px;">"History"</h3>
                <For
                    each=history_list
                    key=|(i, _)| *i
                    children=move |(_, entry)| {
                        view! {
                            <div style="padding: 6px 0; border-bottom: 1px solid #1a1a2e; \
                                        font-size: 14px; color: #888;">
                                {entry}
                            </div>
                        }
                    }
                />
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
