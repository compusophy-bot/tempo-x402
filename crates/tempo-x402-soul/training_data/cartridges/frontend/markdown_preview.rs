use leptos::*;
use wasm_bindgen::prelude::*;

fn markdown_to_html(input: &str) -> String {
    let mut output = String::new();
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("### ") {
            output.push_str(&format!(
                "<h3 style=\"color:#c792ea;font-size:18px;margin:12px 0 6px;\">{}</h3>",
                escape_html(&trimmed[4..])
            ));
        } else if trimmed.starts_with("## ") {
            output.push_str(&format!(
                "<h2 style=\"color:#e5c07b;font-size:22px;margin:14px 0 8px;\">{}</h2>",
                escape_html(&trimmed[3..])
            ));
        } else if trimmed.starts_with("# ") {
            output.push_str(&format!(
                "<h1 style=\"color:#7fdbca;font-size:28px;margin:16px 0 10px;\">{}</h1>",
                escape_html(&trimmed[2..])
            ));
        } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            output.push_str(&format!(
                "<div style=\"margin:2px 0 2px 20px;\">\u{2022} {}</div>",
                inline_format(&trimmed[2..])
            ));
        } else if trimmed.starts_with("> ") {
            output.push_str(&format!(
                "<blockquote style=\"border-left:3px solid #444;padding:4px 12px;margin:8px 0;\
                 color:#888;font-style:italic;\">{}</blockquote>",
                inline_format(&trimmed[2..])
            ));
        } else if trimmed.starts_with("```") {
            output.push_str("<pre style=\"background:#0d1117;padding:12px;border-radius:6px;\
                             font-family:monospace;font-size:13px;margin:8px 0;overflow-x:auto;\">");
        } else if trimmed == "---" || trimmed == "***" {
            output.push_str("<hr style=\"border:none;border-top:1px solid #333;margin:12px 0;\"/>");
        } else if trimmed.is_empty() {
            output.push_str("<br/>");
        } else {
            output.push_str(&format!(
                "<p style=\"margin:6px 0;line-height:1.6;\">{}</p>",
                inline_format(trimmed)
            ));
        }
    }
    output
}

fn inline_format(text: &str) -> String {
    let escaped = escape_html(text);
    let mut result = escaped;

    // Bold: **text**
    while let Some(start) = result.find("**") {
        if let Some(end) = result[start + 2..].find("**") {
            let before = &result[..start];
            let bold_text = &result[start + 2..start + 2 + end];
            let after = &result[start + 2 + end + 2..];
            result = format!(
                "{}<strong style=\"color:#e0e0e0;\">{}</strong>{}",
                before, bold_text, after
            );
        } else {
            break;
        }
    }

    // Italic: *text*
    while let Some(start) = result.find('*') {
        if let Some(end) = result[start + 1..].find('*') {
            let before = &result[..start];
            let italic_text = &result[start + 1..start + 1 + end];
            let after = &result[start + 1 + end + 1..];
            result = format!(
                "{}<em style=\"color:#d19a66;\">{}</em>{}",
                before, italic_text, after
            );
        } else {
            break;
        }
    }

    // Inline code: `text`
    while let Some(start) = result.find('`') {
        if let Some(end) = result[start + 1..].find('`') {
            let before = &result[..start];
            let code_text = &result[start + 1..start + 1 + end];
            let after = &result[start + 1 + end + 1..];
            result = format!(
                "{}<code style=\"background:#1a1a2e;padding:2px 6px;border-radius:3px;\
                 font-family:monospace;font-size:13px;\">{}</code>{}",
                before, code_text, after
            );
        } else {
            break;
        }
    }

    result
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
}

#[component]
fn App() -> impl IntoView {
    let sample = "# Markdown Preview\n\n## Features\n\nThis previewer supports **bold**, \
                  *italic*, and `inline code`.\n\n### Lists\n\n- First item\n- Second item\n\
                  - Third item\n\n> This is a blockquote\n\n---\n\nType in the left panel to \
                  see the preview update in real time.";

    let (input, set_input) = create_signal(sample.to_string());

    let rendered = move || markdown_to_html(&input.get());

    let textarea_style = "width: 100%; height: 100%; min-height: 400px; padding: 14px; \
                          background: #0d1117; border: 1px solid #222; color: #e0e0e0; \
                          border-radius: 8px; font-size: 14px; outline: none; resize: none; \
                          font-family: 'Courier New', monospace; line-height: 1.5;";

    view! {
        <div style="font-family: 'Segoe UI', sans-serif; background: #0a0a0a; color: #e0e0e0; \
                     min-height: 100vh; display: flex; flex-direction: column; align-items: center; \
                     padding: 30px 20px; gap: 20px;">
            <h1 style="font-size: 28px; color: #7fdbca; margin: 0;">"Markdown Preview"</h1>

            <div style="display: flex; gap: 16px; width: 100%; max-width: 1000px; \
                        min-height: 450px;">
                <div style="flex: 1; display: flex; flex-direction: column; gap: 6px;">
                    <span style="font-size: 12px; color: #666; text-transform: uppercase; \
                                 letter-spacing: 1px;">"Markdown"</span>
                    <textarea
                        style=textarea_style
                        prop:value=input
                        on:input=move |ev| set_input.set(event_target_value(&ev))
                    ></textarea>
                </div>
                <div style="flex: 1; display: flex; flex-direction: column; gap: 6px;">
                    <span style="font-size: 12px; color: #666; text-transform: uppercase; \
                                 letter-spacing: 1px;">"Preview"</span>
                    <div
                        style="flex: 1; padding: 14px; background: #111; border: 1px solid #222; \
                               border-radius: 8px; overflow-y: auto; font-size: 14px; \
                               line-height: 1.6;"
                        inner_html=rendered
                    ></div>
                </div>
            </div>

            <div style="color: #555; font-size: 12px;">
                {move || {
                    let t = input.get();
                    let words = if t.trim().is_empty() { 0 } else { t.split_whitespace().count() };
                    format!("{} characters | {} words | {} lines", t.len(), words, t.lines().count())
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
