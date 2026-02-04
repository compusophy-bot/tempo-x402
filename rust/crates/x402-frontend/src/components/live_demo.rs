use leptos::prelude::*;
use serde::Deserialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[derive(Debug, Clone, Deserialize)]
struct SseEvent {
    #[serde(default)]
    r#type: String,
    #[serde(default)]
    step: Option<String>,
    #[serde(default)]
    detail: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    payer: Option<String>,
    #[serde(default)]
    tx: Option<String>,
    #[serde(default)]
    data: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    transaction: Option<String>,
    #[serde(rename = "explorerUrl", default)]
    explorer_url: Option<String>,
}

#[derive(Debug, Clone)]
struct DemoStep {
    step: String,
    detail: String,
    status: String,
    payer: Option<String>,
    data: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
struct DemoResult {
    transaction: String,
    explorer_url: String,
}

fn step_label(step: &str) -> &str {
    match step {
        "request" => "Initial Request",
        "payment_required" => "Payment Required",
        "sign" => "Sign Payment",
        "verify" => "Verify",
        "settle" => "Settle On-Chain",
        "response" => "Response",
        _ => step,
    }
}

fn step_icon(status: &str) -> &str {
    if status == "ok" { "\u{2713}" } else { "\u{2717}" }
}

fn trunc_addr(addr: &str) -> String {
    if addr.len() < 12 {
        addr.to_string()
    } else {
        format!("{}\u{2026}{}", &addr[..6], &addr[addr.len()-4..])
    }
}

#[component]
pub fn LiveDemo() -> impl IntoView {
    let (running, set_running) = signal(false);
    let (steps, set_steps) = signal(Vec::<DemoStep>::new());
    let (result, set_result) = signal(Option::<DemoResult>::None);
    let (error, set_error) = signal(Option::<String>::None);
    let (has_run, set_has_run) = signal(false);

    let on_click = move |_| {
        set_running.set(true);
        set_steps.set(Vec::new());
        set_result.set(None);
        set_error.set(None);

        spawn_local(async move {
            match stream_demo(set_steps, set_result, set_error).await {
                Ok(()) => {}
                Err(e) => {
                    set_error.set(Some(format!("Network error: {e}")));
                }
            }
            set_running.set(false);
            set_has_run.set(true);
        });
    };

    view! {
            <div class="card demo-box">
                <button
                    class="demo-btn"
                    disabled=move || running.get()
                    on:click=on_click
                >
                    {move || {
                        if running.get() {
                            "Running..."
                        } else if has_run.get() {
                            "Run Demo Again"
                        } else {
                            "Run Demo"
                        }
                    }}
                </button>

                <div class="demo-output">
                    {move || steps.get().into_iter().map(|s| {
                        let is_ok = s.status == "ok";
                        let icon_class = if is_ok { "ok" } else { "err" };
                        let icon = step_icon(&s.status).to_string();
                        let label = step_label(&s.step).to_string();
                        let detail = s.detail.clone();
                        let payer_str = s.payer.as_ref().map(|p| format!(" \u{00B7} {}", trunc_addr(p))).unwrap_or_default();
                        let status_class = if is_ok { "ok" } else { "err" };
                        let status_text = s.status.clone();

                        view! {
                            <div class="demo-step">
                                <div class={format!("demo-step-icon {icon_class}")}>{icon}</div>
                                <div class="demo-step-label">
                                    <strong>{label}</strong>
                                    <span>{detail}{payer_str}</span>
                                </div>
                                <div class={format!("demo-step-status {status_class}")}>{status_text}</div>
                            </div>
                        }
                    }).collect::<Vec<_>>()}

                    {move || {
                        if running.get() && result.get().is_none() && error.get().is_none() {
                            Some(view! {
                                <div class="demo-step demo-step-loading">
                                    <div class="demo-step-icon loading">
                                        <div class="spinner"></div>
                                    </div>
                                    <div class="demo-step-label">
                                        <strong>
                                            {move || {
                                                let s = steps.get();
                                                match s.last().map(|l| l.step.as_str()) {
                                                    None | Some("request") => "Waiting for server...",
                                                    Some("payment_required") => "Signing payment...",
                                                    Some("sign") => "Verifying with facilitator...",
                                                    Some("verify") => "Settling on-chain...",
                                                    Some("settle") => "Fetching response...",
                                                    _ => "Processing...",
                                                }
                                            }}
                                        </strong>
                                    </div>
                                </div>
                            })
                        } else {
                            None
                        }
                    }}

                    {move || result.get().map(|data| {
                        let tx_short = trunc_addr(&data.transaction);
                        let explorer = data.explorer_url.clone();
                        let explorer2 = data.explorer_url.clone();
                        let explorer3 = data.explorer_url.clone();
                        let block_num = steps.get().iter()
                            .find_map(|s| s.data.as_ref())
                            .and_then(|d| d.get("blockNumber"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("-")
                            .to_string();

                        view! {
                            <div class="demo-result">
                                <h4>"Payment Settled"</h4>
                                <div class="result-row">
                                    <span class="label">"Block Number"</span>
                                    <span class="value">{block_num}</span>
                                </div>
                                <div class="result-row">
                                    <span class="label">"Transaction"</span>
                                    <span class="value">
                                        <a href={explorer} target="_blank" rel="noopener">{tx_short}</a>
                                    </span>
                                </div>
                                <div class="result-row">
                                    <span class="label">"Explorer"</span>
                                    <span class="value">
                                        <a href={explorer2} target="_blank" rel="noopener">{explorer3}</a>
                                    </span>
                                </div>
                            </div>
                        }
                    })}

                    {move || error.get().map(|e| {
                        view! {
                            <div class="demo-error">{"Error: "}{e}</div>
                        }
                    })}
                </div>
            </div>
    }
}

async fn stream_demo(
    set_steps: WriteSignal<Vec<DemoStep>>,
    set_result: WriteSignal<Option<DemoResult>>,
    set_error: WriteSignal<Option<String>>,
) -> Result<(), String> {
    // Use fetch API with streaming reader for SSE
    let window = web_sys::window().ok_or("no window")?;
    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");
    let request = web_sys::Request::new_with_str_and_init("/api/demo", &opts)
        .map_err(|e| format!("{e:?}"))?;

    let resp: web_sys::Response = wasm_bindgen_futures::JsFuture::from(
        window.fetch_with_request(&request)
    ).await.map_err(|e| format!("{e:?}"))?.unchecked_into();

    let body = resp.body().ok_or("no body")?;
    let reader: web_sys::ReadableStreamDefaultReader = body.get_reader().unchecked_into();
    let decoder = web_sys::TextDecoder::new().map_err(|e| format!("{e:?}"))?;

    let mut buffer = String::new();

    loop {
        let chunk: JsValue = wasm_bindgen_futures::JsFuture::from(reader.read())
            .await
            .map_err(|e| format!("{e:?}"))?;

        let done = js_sys::Reflect::get(&chunk, &JsValue::from_str("done"))
            .unwrap_or(JsValue::TRUE)
            .as_bool()
            .unwrap_or(true);

        if done {
            break;
        }

        let value = js_sys::Reflect::get(&chunk, &JsValue::from_str("value"))
            .map_err(|e| format!("{e:?}"))?;
        let array: js_sys::Uint8Array = value.unchecked_into();
        let text = decoder.decode_with_buffer_source(&array)
            .map_err(|e| format!("{e:?}"))?;

        buffer.push_str(&text);

        // Process complete SSE events (delimited by \n\n)
        while let Some(pos) = buffer.find("\n\n") {
            let event_str = buffer[..pos].to_string();
            buffer = buffer[pos + 2..].to_string();

            // SSE format: "data: {...}"
            let json_str = event_str
                .strip_prefix("data: ")
                .unwrap_or(&event_str);

            if let Ok(event) = serde_json::from_str::<SseEvent>(json_str) {
                match event.r#type.as_str() {
                    "step" => {
                        let step = DemoStep {
                            step: event.step.unwrap_or_default(),
                            detail: event.detail.unwrap_or_default(),
                            status: event.status.unwrap_or_default(),
                            payer: event.payer,
                            data: event.data,
                        };
                        set_steps.update(|s| s.push(step));
                    }
                    "done" => {
                        set_result.set(Some(DemoResult {
                            transaction: event.transaction.unwrap_or_default(),
                            explorer_url: event.explorer_url.unwrap_or_default(),
                        }));
                    }
                    "error" => {
                        if let Some(err) = event.error {
                            set_error.set(Some(err));
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
