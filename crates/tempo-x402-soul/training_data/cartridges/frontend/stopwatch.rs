use leptos::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::closure::Closure;

#[component]
fn App() -> impl IntoView {
    let (elapsed_ms, set_elapsed_ms) = create_signal(0u64);
    let (running, set_running) = create_signal(false);
    let (laps, set_laps) = create_signal(Vec::<u64>::new());
    let (interval_id, set_interval_id) = create_signal(Option::<i32>::None);

    let format_time = |ms: u64| -> String {
        let total_secs = ms / 1000;
        let hours = total_secs / 3600;
        let mins = (total_secs % 3600) / 60;
        let secs = total_secs % 60;
        let centis = (ms % 1000) / 10;
        if hours > 0 {
            format!("{:02}:{:02}:{:02}.{:02}", hours, mins, secs, centis)
        } else {
            format!("{:02}:{:02}.{:02}", mins, secs, centis)
        }
    };

    let start_timer = move || {
        let window = web_sys::window().unwrap();
        let cb = Closure::<dyn Fn()>::new(move || {
            set_elapsed_ms.update(|ms| *ms += 10);
        });
        let id = window.set_interval_with_callback_and_timeout_and_arguments_0(
            cb.as_ref().unchecked_ref(), 10
        ).unwrap();
        cb.forget();
        set_interval_id.set(Some(id));
    };

    let stop_timer = move || {
        if let Some(id) = interval_id.get() {
            web_sys::window().unwrap().clear_interval_with_handle(id);
            set_interval_id.set(None);
        }
    };

    let toggle = move |_| {
        if running.get() {
            stop_timer();
            set_running.set(false);
        } else {
            start_timer();
            set_running.set(true);
        }
    };

    let lap = move |_| {
        if running.get() {
            let current = elapsed_ms.get();
            set_laps.update(|l| l.push(current));
        }
    };

    let reset = move |_| {
        stop_timer();
        set_running.set(false);
        set_elapsed_ms.set(0);
        set_laps.set(Vec::new());
    };

    let btn_style = "padding: 12px 24px; font-size: 16px; border: 1px solid #333; \
                     cursor: pointer; border-radius: 8px; min-width: 80px;";

    view! {
        <div style="font-family: 'Segoe UI', sans-serif; background: #0a0a0a; color: #e0e0e0; \
                     min-height: 100vh; display: flex; flex-direction: column; align-items: center; \
                     justify-content: center; gap: 24px;">
            <h1 style="font-size: 28px; color: #7fdbca; margin: 0;">"Stopwatch"</h1>

            <div style="font-size: 64px; font-weight: bold; color: #c792ea; \
                        font-variant-numeric: tabular-nums; letter-spacing: 2px;">
                {move || format_time(elapsed_ms.get())}
            </div>

            <div style="display: flex; gap: 12px;">
                <button
                    style=move || format!("{} background: {}; color: {};",
                        btn_style,
                        if running.get() { "#3a1a1a" } else { "#1a3a1a" },
                        if running.get() { "#e06c75" } else { "#98c379" }
                    )
                    on:click=toggle
                >
                    {move || if running.get() { "Stop" } else { "Start" }}
                </button>
                <button
                    style=format!("{} background: #1a1a2e; color: #e0e0e0;", btn_style)
                    on:click=lap
                >"Lap"</button>
                <button
                    style=format!("{} background: #1a1a2e; color: #e0e0e0;", btn_style)
                    on:click=reset
                >"Reset"</button>
            </div>

            <Show when=move || !laps.get().is_empty()>
                <div style="width: 100%; max-width: 400px; margin-top: 12px;">
                    <h3 style="color: #666; font-size: 14px; margin-bottom: 8px;">"Laps"</h3>
                    <div style="display: flex; flex-direction: column; gap: 4px;">
                        <For
                            each=move || {
                                let l = laps.get();
                                let mut items = Vec::new();
                                for i in 0..l.len() {
                                    let prev = if i == 0 { 0 } else { l[i - 1] };
                                    items.push((i + 1, l[i] - prev, l[i]));
                                }
                                items.reverse();
                                items
                            }
                            key=|item| item.0
                            children=move |(num, split, total)| {
                                let ft = format_time;
                                view! {
                                    <div style="display: flex; justify-content: space-between; \
                                                padding: 8px 12px; background: #111; border-radius: 4px; \
                                                font-variant-numeric: tabular-nums; font-size: 14px;">
                                        <span style="color: #666;">{format!("Lap {}", num)}</span>
                                        <span style="color: #e5c07b;">{ft(split)}</span>
                                        <span style="color: #7fdbca;">{ft(total)}</span>
                                    </div>
                                }
                            }
                        />
                    </div>
                </div>
            </Show>
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
