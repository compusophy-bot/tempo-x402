use crate::api;
use gloo_timers::callback::Interval;
use leptos::*;

/// Chat display message for the UI
#[derive(Clone, Debug)]
struct ChatDisplayMessage {
    role: &'static str, // "user" or "soul"
    content: String,
    tool_executions: Vec<serde_json::Value>,
    timestamp: i64,
    hemisphere: Option<String>,
}

/// Floating chat widget — bottom-right FAB that expands into a chat panel
#[component]
pub fn ChatWidget() -> impl IntoView {
    let (open, set_open) = create_signal(false);
    let (messages, set_messages) = create_signal(Vec::<ChatDisplayMessage>::new());
    let (input, set_input) = create_signal(String::new());
    let (loading, set_loading) = create_signal(false);
    let (error, set_error) = create_signal(None::<String>);
    let (session_id, set_session_id) = create_signal(None::<String>);
    let (pending_plan, set_pending_plan) = create_signal(None::<serde_json::Value>);
    let messages_ref = create_node_ref::<html::Div>();

    let scroll_to_bottom = move || {
        if let Some(el) = messages_ref.get() {
            el.set_scroll_top(el.scroll_height());
        }
    };

    // Fetch pending plan on mount + every 10s
    {
        spawn_local(async move {
            loop {
                if let Ok(plan) = api::get_pending_plan().await {
                    if plan.is_null() {
                        set_pending_plan.set(None);
                    } else {
                        set_pending_plan.set(Some(plan));
                    }
                }
                gloo_timers::future::TimeoutFuture::new(10_000).await;
            }
        });
    }

    let now_ts = move || (js_sys::Date::now() / 1000.0) as i64;

    let do_send = move || {
        let msg = input.get().trim().to_string();
        if msg.is_empty() || loading.get() {
            return;
        }

        let ts = now_ts();
        set_messages.update(|msgs| {
            msgs.push(ChatDisplayMessage {
                role: "user",
                content: msg.clone(),
                tool_executions: vec![],
                timestamp: ts,
                hemisphere: None,
            });
        });
        set_input.set(String::new());
        set_loading.set(true);
        set_error.set(None);

        let sid = session_id.get();
        spawn_local(async move {
            let result = api::send_soul_chat(&msg, sid.as_deref()).await;

            match result {
                Ok(resp) => {
                    // Track session_id from response
                    if let Some(sid) = resp.get("session_id").and_then(|v| v.as_str()) {
                        set_session_id.set(Some(sid.to_string()));
                    }

                    let reply = resp
                        .get("reply")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let tools = resp
                        .get("tool_executions")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    let hemisphere = resp
                        .get("hemisphere")
                        .and_then(|v| v.as_str())
                        .map(String::from);

                    let ts = (js_sys::Date::now() / 1000.0) as i64;
                    set_messages.update(|msgs| {
                        msgs.push(ChatDisplayMessage {
                            role: "soul",
                            content: reply,
                            tool_executions: tools,
                            timestamp: ts,
                            hemisphere,
                        });
                    });
                }
                Err(e) => {
                    set_error.set(Some(e));
                }
            }
            set_loading.set(false);
            scroll_to_bottom();
        });

        // Scroll after adding user message
        scroll_to_bottom();
    };

    let clear_chat = move |_| {
        set_messages.set(Vec::new());
        set_error.set(None);
        set_session_id.set(None); // Start a new session
    };

    let on_click = move |_: web_sys::MouseEvent| {
        do_send();
    };

    let on_keydown = move |ev: web_sys::KeyboardEvent| {
        if ev.key() == "Enter" && !ev.shift_key() {
            ev.prevent_default();
            do_send();
        }
    };

    let approve_handler = move |_: web_sys::MouseEvent| {
        if let Some(plan) = pending_plan.get() {
            if let Some(plan_id) = plan.get("id").and_then(|v| v.as_str()) {
                let plan_id = plan_id.to_string();
                spawn_local(async move {
                    let _ = api::approve_plan(&plan_id).await;
                    set_pending_plan.set(None);
                });
            }
        }
    };

    let reject_handler = move |_: web_sys::MouseEvent| {
        if let Some(plan) = pending_plan.get() {
            if let Some(plan_id) = plan.get("id").and_then(|v| v.as_str()) {
                let plan_id = plan_id.to_string();
                spawn_local(async move {
                    let _ = api::reject_plan(&plan_id, None).await;
                    set_pending_plan.set(None);
                });
            }
        }
    };

    let toggle_chat = move |_: web_sys::MouseEvent| set_open.update(|v| *v = !*v);

    view! {
        <div class="chat-widget">
            <button class="chat-fab" on:click=toggle_chat title="Chat with Soul">
                {move || if open.get() { "\u{2715}" } else { "\u{1F4AC}" }}
            </button>

            <Show when=move || open.get() fallback=|| ()>
            <div class="chat-panel">
                <div class="chat-panel-header">
                    <span>"Soul Chat"</span>
                    <button class="chat-clear-btn" on:click=clear_chat>"New"</button>
                </div>

                <Show when=move || pending_plan.get().is_some() fallback=|| ()>
                    <div class="plan-approval-bar plan-approval-bar--widget">
                        <div class="plan-approval-info">
                            <span class="plan-approval-badge">"PLAN AWAITING APPROVAL"</span>
                            <span class="plan-approval-desc">
                                {move || pending_plan.get()
                                    .and_then(|p| p.get("goal_description")
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string()))
                                    .unwrap_or_else(|| "Unknown goal".to_string())
                                }
                            </span>
                        </div>
                        <div class="plan-approval-actions">
                            <button class="btn btn-approve btn--sm" on:click=approve_handler>"Approve"</button>
                            <button class="btn btn-reject btn--sm" on:click=reject_handler>"Reject"</button>
                        </div>
                    </div>
                </Show>

                <div class="chat-messages" node_ref=messages_ref>
                <For
                    each=move || {
                        let msgs = messages.get();
                        msgs.into_iter().enumerate().collect::<Vec<_>>()
                    }
                    key=|(i, _)| *i
                    children=move |(_, msg)| {
                        let class = format!("chat-message {}", msg.role);
                        let tools = msg.tool_executions.clone();
                        let ts = msg.timestamp;
                        let hemisphere = msg.hemisphere.clone();
                        view! {
                            <div class=class>
                                <div class="chat-message-header">
                                    <div style="display:flex;gap:6px;align-items:center">
                                        <span class="chat-message-role">
                                            {if msg.role == "user" { "You" } else { "Soul" }}
                                        </span>
                                        {hemisphere.map(|h| view! {
                                            <span class="chat-hemisphere-tag">{h}</span>
                                        })}
                                    </div>
                                    <span class="chat-message-time">{format_timestamp(ts)}</span>
                                </div>
                                <div class="chat-message-content">
                                    {msg.content.clone()}
                                </div>
                                {if !tools.is_empty() {
                                    Some(view! {
                                        <div class="chat-tools">
                                            {tools.into_iter().map(|t| {
                                                let cmd = t.get("command")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("?")
                                                    .to_string();
                                                let stdout = t.get("stdout")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                let stderr = t.get("stderr")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                let exit_code = t.get("exit_code")
                                                    .and_then(|v| v.as_i64())
                                                    .unwrap_or(-1);
                                                let (expanded, set_expanded) = create_signal(false);
                                                view! {
                                                    <div class="chat-tool-block">
                                                        <button
                                                            class="chat-tool-header"
                                                            on:click=move |_| set_expanded.update(|v| *v = !*v)
                                                        >
                                                            <span class="chat-tool-cmd">"$ " {cmd.clone()}</span>
                                                            <span class="chat-tool-exit">
                                                                {format!("exit {}", exit_code)}
                                                            </span>
                                                        </button>
                                                        <Show when=move || expanded.get() fallback=|| ()>
                                                            <pre class="chat-tool-output">
                                                                {if !stdout.is_empty() {
                                                                    stdout.clone()
                                                                } else if !stderr.is_empty() {
                                                                    stderr.clone()
                                                                } else {
                                                                    "(no output)".to_string()
                                                                }}
                                                            </pre>
                                                        </Show>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    })
                                } else {
                                    None
                                }}
                            </div>
                        }
                    }
                />

                <Show when=move || loading.get() fallback=|| ()>
                    <div class="chat-message soul">
                        <div class="chat-message-role">"Soul"</div>
                        <div class="chat-message-content chat-typing">"Thinking..."</div>
                    </div>
                </Show>

                <Show when=move || error.get().is_some() fallback=|| ()>
                    <div class="chat-error">
                        {move || error.get().unwrap_or_default()}
                    </div>
                </Show>
            </div>

                <div class="chat-input-bar">
                    <input
                        type="text"
                        class="chat-input"
                        placeholder="Ask the soul something..."
                        prop:value=move || input.get()
                        on:input=move |ev| set_input.set(event_target_value(&ev))
                        on:keydown=on_keydown
                        disabled=move || loading.get()
                    />
                    <button
                        class="btn btn-primary btn--sm"
                        on:click=on_click
                        disabled=move || loading.get() || input.get().trim().is_empty()
                    >
                        {move || if loading.get() { "..." } else { "Send" }}
                    </button>
                </div>
            </div>
            </Show>
        </div>
    }
}

/// Format a unix timestamp as HH:MM
fn format_timestamp(unix_ts: i64) -> String {
    let date = js_sys::Date::new_0();
    date.set_time((unix_ts as f64) * 1000.0);
    let h = date.get_hours();
    let m = date.get_minutes();
    format!("{:02}:{:02}", h, m)
}
