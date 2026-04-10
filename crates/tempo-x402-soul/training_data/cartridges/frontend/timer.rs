use leptos::*;
use wasm_bindgen::prelude::*;

#[component]
fn App() -> impl IntoView {
    let (hours, set_hours) = create_signal(0u32);
    let (minutes, set_minutes) = create_signal(5u32);
    let (seconds, set_seconds) = create_signal(0u32);
    let (remaining, set_remaining) = create_signal(0u32);
    let (running, set_running) = create_signal(false);
    let (interval_id, set_interval_id) = create_signal(None::<i32>);

    let format_time = move |total_secs: u32| -> String {
        let h = total_secs / 3600;
        let m = (total_secs % 3600) / 60;
        let s = total_secs % 60;
        format!("{:02}:{:02}:{:02}", h, m, s)
    };

    let start = move |_| {
        if running.get() { return; }
        let total = hours.get() * 3600 + minutes.get() * 60 + seconds.get();
        if total == 0 { return; }
        set_remaining.set(total);
        set_running.set(true);

        let cb = Closure::<dyn FnMut()>::new(move || {
            let r = remaining.get();
            if r > 0 {
                set_remaining.set(r - 1);
            } else {
                set_running.set(false);
                if let Some(id) = interval_id.get() {
                    web_sys::window().unwrap().clear_interval_with_handle(id);
                    set_interval_id.set(None);
                }
            }
        });

        let window = web_sys::window().unwrap();
        let id = window.set_interval_with_callback_and_timeout_and_arguments_0(
            cb.as_ref().unchecked_ref(), 1000
        ).unwrap();
        cb.forget();
        set_interval_id.set(Some(id));
    };

    let stop = move |_| {
        set_running.set(false);
        if let Some(id) = interval_id.get() {
            web_sys::window().unwrap().clear_interval_with_handle(id);
            set_interval_id.set(None);
        }
    };

    let reset = move |_| {
        set_running.set(false);
        set_remaining.set(0);
        if let Some(id) = interval_id.get() {
            web_sys::window().unwrap().clear_interval_with_handle(id);
            set_interval_id.set(None);
        }
    };

    let presets = vec![(1, "1 min"), (5, "5 min"), (10, "10 min"), (15, "15 min"), (30, "30 min")];

    view! {
        <div style="font-family: 'Courier New', monospace; background: #000; color: #e0e0e0; \
                     min-height: 100vh; display: flex; flex-direction: column; align-items: center; \
                     justify-content: center; padding: 20px;">
            <h1 style="color: #00ff88; margin-bottom: 30px; font-size: 1.5em;">"TIMER"</h1>

            <div style="font-size: 72px; color: #00ff88; font-weight: bold; margin-bottom: 30px; \
                        text-shadow: 0 0 30px rgba(0,255,136,0.3); letter-spacing: 8px;">
                {move || format_time(remaining.get())}
            </div>

            {move || {
                if !running.get() && remaining.get() == 0 {
                    view! {
                        <div style="display: flex; gap: 12px; margin-bottom: 20px; align-items: center;">
                            <div style="text-align: center;">
                                <label style="font-size: 11px; color: #666;">"HRS"</label>
                                <input type="number" min="0" max="23"
                                    prop:value=move || hours.get().to_string()
                                    on:input=move |ev| { if let Ok(v) = event_target_value(&ev).parse() { set_hours.set(v); } }
                                    style="width: 60px; padding: 10px; background: #111; border: 1px solid #333; \
                                           color: #00ff88; border-radius: 6px; text-align: center; font-size: 18px; \
                                           font-family: 'Courier New', monospace;"
                                />
                            </div>
                            <span style="font-size: 24px; color: #333;">":"</span>
                            <div style="text-align: center;">
                                <label style="font-size: 11px; color: #666;">"MIN"</label>
                                <input type="number" min="0" max="59"
                                    prop:value=move || minutes.get().to_string()
                                    on:input=move |ev| { if let Ok(v) = event_target_value(&ev).parse() { set_minutes.set(v); } }
                                    style="width: 60px; padding: 10px; background: #111; border: 1px solid #333; \
                                           color: #00ff88; border-radius: 6px; text-align: center; font-size: 18px; \
                                           font-family: 'Courier New', monospace;"
                                />
                            </div>
                            <span style="font-size: 24px; color: #333;">":"</span>
                            <div style="text-align: center;">
                                <label style="font-size: 11px; color: #666;">"SEC"</label>
                                <input type="number" min="0" max="59"
                                    prop:value=move || seconds.get().to_string()
                                    on:input=move |ev| { if let Ok(v) = event_target_value(&ev).parse() { set_seconds.set(v); } }
                                    style="width: 60px; padding: 10px; background: #111; border: 1px solid #333; \
                                           color: #00ff88; border-radius: 6px; text-align: center; font-size: 18px; \
                                           font-family: 'Courier New', monospace;"
                                />
                            </div>
                        </div>
                    }.into_view()
                } else {
                    view! { <div></div> }.into_view()
                }
            }}

            <div style="display: flex; gap: 8px; margin-bottom: 20px; flex-wrap: wrap; justify-content: center;">
                {presets.into_iter().map(|(mins, label)| {
                    view! {
                        <button
                            on:click=move |_| { set_hours.set(0); set_minutes.set(mins); set_seconds.set(0); }
                            style="padding: 8px 16px; border: 1px solid #333; background: #111; \
                                   color: #00ff88; border-radius: 20px; cursor: pointer; font-size: 13px; \
                                   font-family: 'Courier New', monospace;"
                        >{label}</button>
                    }
                }).collect::<Vec<_>>()}
            </div>

            <div style="display: flex; gap: 12px;">
                <button
                    on:click=start
                    style="padding: 14px 32px; border: 2px solid #00ff88; background: transparent; \
                           color: #00ff88; border-radius: 30px; cursor: pointer; font-size: 16px; \
                           font-family: 'Courier New', monospace;"
                >"START"</button>
                <button
                    on:click=stop
                    style="padding: 14px 32px; border: 2px solid #ff4444; background: transparent; \
                           color: #ff4444; border-radius: 30px; cursor: pointer; font-size: 16px; \
                           font-family: 'Courier New', monospace;"
                >"STOP"</button>
                <button
                    on:click=reset
                    style="padding: 14px 32px; border: 2px solid #666; background: transparent; \
                           color: #666; border-radius: 30px; cursor: pointer; font-size: 16px; \
                           font-family: 'Courier New', monospace;"
                >"RESET"</button>
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
