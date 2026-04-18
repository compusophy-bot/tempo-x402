use leptos::*;
use wasm_bindgen::prelude::*;

#[component]
fn App() -> impl IntoView {
    let (text, set_text) = create_signal(String::new());

    let char_count = move || text.get().len();
    let char_no_spaces = move || text.get().chars().filter(|c| !c.is_whitespace()).count();
    let word_count = move || {
        let t = text.get();
        if t.trim().is_empty() { 0 } else { t.split_whitespace().count() }
    };
    let line_count = move || {
        let t = text.get();
        if t.is_empty() { 0 } else { t.lines().count() }
    };
    let sentence_count = move || {
        let t = text.get();
        t.chars().filter(|c| *c == '.' || *c == '!' || *c == '?').count()
    };
    let avg_word_len = move || {
        let wc = word_count();
        if wc == 0 { 0.0 } else {
            let total: usize = text.get().split_whitespace().map(|w| w.len()).sum();
            total as f64 / wc as f64
        }
    };

    let stat_style = "display: flex; flex-direction: column; align-items: center; \
                      padding: 16px; background: #111; border-radius: 8px; min-width: 100px;";
    let stat_num = "font-size: 32px; font-weight: bold; color: #c792ea; \
                    font-variant-numeric: tabular-nums;";
    let stat_label = "font-size: 12px; color: #666; text-transform: uppercase; \
                      letter-spacing: 1px; margin-top: 4px;";

    view! {
        <div style="font-family: 'Segoe UI', sans-serif; background: #0a0a0a; color: #e0e0e0; \
                     min-height: 100vh; display: flex; flex-direction: column; align-items: center; \
                     padding: 40px 20px; gap: 24px;">
            <h1 style="font-size: 28px; color: #7fdbca; margin: 0;">"Character Counter"</h1>

            <textarea
                style="width: 100%; max-width: 600px; min-height: 200px; padding: 14px; \
                       background: #111; border: 1px solid #333; color: #e0e0e0; \
                       border-radius: 8px; font-size: 15px; outline: none; resize: vertical; \
                       font-family: 'Segoe UI', sans-serif; line-height: 1.6;"
                placeholder="Start typing or paste text here..."
                prop:value=text
                on:input=move |ev| set_text.set(event_target_value(&ev))
            ></textarea>

            <div style="display: flex; flex-wrap: wrap; gap: 12px; justify-content: center; \
                        width: 100%; max-width: 600px;">
                <div style=stat_style>
                    <span style=stat_num>{char_count}</span>
                    <span style=stat_label>"Characters"</span>
                </div>
                <div style=stat_style>
                    <span style=stat_num>{char_no_spaces}</span>
                    <span style=stat_label>"No Spaces"</span>
                </div>
                <div style=stat_style>
                    <span style=stat_num>{word_count}</span>
                    <span style=stat_label>"Words"</span>
                </div>
                <div style=stat_style>
                    <span style=stat_num>{line_count}</span>
                    <span style=stat_label>"Lines"</span>
                </div>
                <div style=stat_style>
                    <span style=stat_num>{sentence_count}</span>
                    <span style=stat_label>"Sentences"</span>
                </div>
            </div>

            <div style="color: #555; font-size: 13px; text-align: center;">
                {move || format!("Avg word length: {:.1} chars | ~{} min read",
                    avg_word_len(), (word_count() + 199) / 200)}
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
