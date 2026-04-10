use leptos::*;
use wasm_bindgen::prelude::*;

fn pretty_print_json(input: &str) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(input)
        .map_err(|e| format!("Parse error: {}", e))?;
    serde_json::to_string_pretty(&value)
        .map_err(|e| format!("Format error: {}", e))
}

fn minify_json(input: &str) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(input)
        .map_err(|e| format!("Parse error: {}", e))?;
    serde_json::to_string(&value)
        .map_err(|e| format!("Format error: {}", e))
}

#[component]
fn App() -> impl IntoView {
    let (input, set_input) = create_signal(String::new());
    let (output, set_output) = create_signal(String::new());
    let (error, set_error) = create_signal(Option::<String>::None);

    let format_json = move |_| {
        let text = input.get();
        if text.trim().is_empty() {
            set_output.set(String::new());
            set_error.set(None);
            return;
        }
        match pretty_print_json(&text) {
            Ok(formatted) => {
                set_output.set(formatted);
                set_error.set(None);
            }
            Err(e) => {
                set_output.set(String::new());
                set_error.set(Some(e));
            }
        }
    };

    let minify = move |_| {
        let text = input.get();
        if text.trim().is_empty() { return; }
        match minify_json(&text) {
            Ok(mini) => {
                set_output.set(mini);
                set_error.set(None);
            }
            Err(e) => {
                set_output.set(String::new());
                set_error.set(Some(e));
            }
        }
    };

    let validate = move |_| {
        let text = input.get();
        if text.trim().is_empty() {
            set_error.set(None);
            return;
        }
        match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(_) => {
                set_error.set(None);
                set_output.set("Valid JSON!".to_string());
            }
            Err(e) => {
                set_error.set(Some(format!("Invalid: {}", e)));
                set_output.set(String::new());
            }
        }
    };

    let clear = move |_| {
        set_input.set(String::new());
        set_output.set(String::new());
        set_error.set(None);
    };

    let sample = move |_| {
        let sample_json = r#"{"name":"Agent","version":9.1,"features":["brain","cortex","genesis"],"config":{"debug":false,"workers":4}}"#;
        set_input.set(sample_json.to_string());
    };

    let textarea_style = "width: 100%; min-height: 200px; padding: 14px; background: #111; \
                          border: 1px solid #333; color: #e0e0e0; border-radius: 8px; \
                          font-size: 13px; outline: none; resize: vertical; \
                          font-family: 'Courier New', monospace; line-height: 1.5; tab-size: 2;";
    let btn_style = "padding: 10px 20px; border: 1px solid #333; background: #1a1a2e; \
                     color: #e0e0e0; cursor: pointer; border-radius: 6px; font-size: 14px;";

    view! {
        <div style="font-family: 'Segoe UI', sans-serif; background: #0a0a0a; color: #e0e0e0; \
                     min-height: 100vh; display: flex; flex-direction: column; align-items: center; \
                     padding: 40px 20px; gap: 20px;">
            <h1 style="font-size: 28px; color: #7fdbca; margin: 0;">"JSON Formatter"</h1>

            <div style="width: 100%; max-width: 700px; display: flex; flex-direction: column; \
                        gap: 16px;">
                <textarea
                    style=textarea_style
                    placeholder="Paste your JSON here..."
                    prop:value=input
                    on:input=move |ev| set_input.set(event_target_value(&ev))
                ></textarea>

                <div style="display: flex; gap: 8px; flex-wrap: wrap;">
                    <button style=format!("{} background: #1a3a3a; color: #7fdbca; border-color: #7fdbca;", btn_style)
                        on:click=format_json>"Format"</button>
                    <button style=btn_style on:click=minify>"Minify"</button>
                    <button style=btn_style on:click=validate>"Validate"</button>
                    <button style=btn_style on:click=sample>"Sample"</button>
                    <button style=btn_style on:click=clear>"Clear"</button>
                </div>

                <Show when=move || error.get().is_some()>
                    <div style="padding: 12px; background: #2a1515; border: 1px solid #e06c75; \
                                border-radius: 8px; color: #e06c75; font-size: 13px; \
                                font-family: monospace;">
                        {move || error.get().unwrap_or_default()}
                    </div>
                </Show>

                <Show when=move || !output.get().is_empty()>
                    <div style="position: relative;">
                        <pre style="width: 100%; padding: 14px; background: #0d1117; \
                                    border: 1px solid #222; color: #98c379; border-radius: 8px; \
                                    font-size: 13px; font-family: 'Courier New', monospace; \
                                    line-height: 1.5; overflow-x: auto; white-space: pre-wrap; \
                                    margin: 0;">
                            {output}
                        </pre>
                    </div>
                </Show>

                <div style="color: #555; font-size: 12px; text-align: center;">
                    {move || {
                        let text = input.get();
                        if text.is_empty() { String::new() }
                        else { format!("{} characters input", text.len()) }
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
