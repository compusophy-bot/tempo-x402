//! Studio — integrated AI IDE for self-editing web applications.
//!
//! Chat with the soul to build features. The soul reads, writes, and commits code.
//! Changes deploy automatically. The app edits itself from within itself.

use leptos::*;
use serde::{Deserialize, Serialize};

use crate::api;

/// Chat message in the studio conversation.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String, // "user" or "assistant"
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<i64>,
}

/// File entry from the workspace.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct FileEntry {
    name: String,
    #[serde(rename = "type")]
    entry_type: String, // "file" or "directory"
    size: Option<u64>,
}

/// Studio page — the self-editing IDE.
#[component]
pub fn StudioPage() -> impl IntoView {
    let (messages, set_messages) = create_signal(Vec::<ChatMessage>::new());
    let (input, set_input) = create_signal(String::new());
    let (sending, set_sending) = create_signal(false);
    let (session_id, set_session_id) = create_signal(None::<String>);
    let (file_content, set_file_content) = create_signal(None::<(String, String)>); // (path, content)
    let (file_tree, set_file_tree) = create_signal(Vec::<FileEntry>::new());
    let (current_path, set_current_path) = create_signal("src".to_string());
    let (deploy_status, set_deploy_status) = create_signal("idle".to_string());
    let (soul_status, set_soul_status) = create_signal(None::<serde_json::Value>);

    // Poll soul status every 10s
    let poll_status = move || {
        spawn_local(async move {
            if let Ok(data) = api::fetch_soul_status().await {
                set_soul_status.set(Some(data));
            }
        });
    };
    poll_status();
    let _interval =
        set_interval_with_handle(move || poll_status(), std::time::Duration::from_secs(10));

    // Load initial file tree
    {
        let set_file_tree = set_file_tree.clone();
        spawn_local(async move {
            if let Ok(files) = fetch_file_tree("src").await {
                set_file_tree.set(files);
            }
        });
    }

    // Send chat message
    let send_message = move || {
        let msg = input.get_untracked();
        if msg.trim().is_empty() || sending.get_untracked() {
            return;
        }

        set_sending.set(true);
        set_input.set(String::new());

        // Add user message immediately
        set_messages.update(|msgs| {
            msgs.push(ChatMessage {
                role: "user".to_string(),
                content: msg.clone(),
                timestamp: None,
            });
        });

        let sid = session_id.get_untracked();
        spawn_local(async move {
            match api::send_soul_chat(&msg, sid.as_deref()).await {
                Ok(resp) => {
                    let reply = resp
                        .get("reply")
                        .and_then(|v| v.as_str())
                        .unwrap_or("(no response)")
                        .to_string();
                    let new_sid = resp
                        .get("session_id")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    if let Some(sid) = new_sid {
                        set_session_id.set(Some(sid));
                    }

                    set_messages.update(|msgs| {
                        msgs.push(ChatMessage {
                            role: "assistant".to_string(),
                            content: reply,
                            timestamp: None,
                        });
                    });
                }
                Err(e) => {
                    set_messages.update(|msgs| {
                        msgs.push(ChatMessage {
                            role: "assistant".to_string(),
                            content: format!("Error: {e}"),
                            timestamp: None,
                        });
                    });
                }
            }
            set_sending.set(false);
        });
    };

    // Handle Enter key
    let send_for_key = send_message.clone();
    let on_keydown = move |ev: web_sys::KeyboardEvent| {
        if ev.key() == "Enter" && !ev.shift_key() {
            ev.prevent_default();
            send_for_key();
        }
    };

    // Load a file when clicked
    let load_file = move |path: String| {
        spawn_local(async move {
            if let Ok(content) = fetch_file_content(&path).await {
                set_file_content.set(Some((path, content)));
            }
        });
    };

    view! {
        <div class="studio">
            <div class="studio-header">
                <h1>"Studio"</h1>
                <div class="studio-status">
                    {move || {
                        let status = soul_status.get();
                        let mode = status.as_ref()
                            .and_then(|d| d.get("mode"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let cycles = status.as_ref()
                            .and_then(|d| d.get("total_cycles"))
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        let coding = status.as_ref()
                            .and_then(|d| d.get("coding_enabled"))
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let benchmark = status.as_ref()
                            .and_then(|d| d.get("benchmark"))
                            .and_then(|b| b.get("opus_iq"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("--");

                        view! {
                            <span class="studio-badge">{format!("Mode: {mode}")}</span>
                            <span class="studio-badge">{format!("Cycles: {cycles}")}</span>
                            <span class="studio-badge">{format!("Coding: {}", if coding { "ON" } else { "OFF" })}</span>
                            <span class="studio-badge">{format!("IQ: {benchmark}")}</span>
                        }
                    }}
                </div>
            </div>

            <div class="studio-layout">
                // File browser (left panel)
                <div class="studio-files">
                    <div class="studio-files-header">
                        <h3>"Files"</h3>
                        <span class="studio-path">{move || current_path.get()}</span>
                    </div>
                    <div class="studio-file-list">
                        {move || {
                            file_tree.get().iter().map(|entry| {
                                let path = format!("{}/{}", current_path.get_untracked(), entry.name);
                                let name = entry.name.clone();
                                let is_dir = entry.entry_type == "directory";
                                let path_for_click = path.clone();
                                let load = load_file.clone();
                                view! {
                                    <div
                                        class=if is_dir { "studio-file studio-file--dir" } else { "studio-file" }
                                        on:click=move |_| {
                                            if is_dir {
                                                let p = path_for_click.clone();
                                                let set_path = set_current_path.clone();
                                                let set_tree = set_file_tree.clone();
                                                spawn_local(async move {
                                                    set_path.set(p.clone());
                                                    if let Ok(files) = fetch_file_tree(&p).await {
                                                        set_tree.set(files);
                                                    }
                                                });
                                            } else {
                                                load(path_for_click.clone());
                                            }
                                        }
                                    >
                                        <span class="studio-file-icon">
                                            {if is_dir { "\u{1F4C1}" } else { "\u{1F4C4}" }}
                                        </span>
                                        <span class="studio-file-name">{name}</span>
                                    </div>
                                }
                            }).collect_view()
                        }}
                    </div>
                </div>

                // Code viewer (center panel)
                <div class="studio-editor">
                    {move || {
                        match file_content.get() {
                            Some((path, content)) => view! {
                                <div class="studio-editor-header">
                                    <span class="studio-editor-path">{path}</span>
                                </div>
                                <pre class="studio-code"><code>{content}</code></pre>
                            }.into_view(),
                            None => view! {
                                <div class="studio-editor-empty">
                                    <p>"Select a file to view, or chat with the soul to make changes."</p>
                                    <p class="studio-hint">
                                        "Try: \"Add a /todo route that shows a simple todo list\""
                                    </p>
                                </div>
                            }.into_view(),
                        }
                    }}
                </div>

                // Chat panel (right panel)
                <div class="studio-chat">
                    <div class="studio-chat-header">
                        <h3>"Chat with Soul"</h3>
                        {move || session_id.get().map(|sid| {
                            view! { <span class="studio-session">{format!("Session: {}...", &sid[..8.min(sid.len())])}</span> }
                        })}
                    </div>
                    <div class="studio-messages">
                        {move || {
                            messages.get().iter().map(|msg| {
                                let is_user = msg.role == "user";
                                let content = msg.content.clone();
                                view! {
                                    <div class=if is_user { "studio-msg studio-msg--user" } else { "studio-msg studio-msg--ai" }>
                                        <div class="studio-msg-role">
                                            {if is_user { "You" } else { "Soul" }}
                                        </div>
                                        <div class="studio-msg-content">
                                            <pre>{content}</pre>
                                        </div>
                                    </div>
                                }
                            }).collect_view()
                        }}
                        {move || {
                            if sending.get() {
                                view! { <div class="studio-msg studio-msg--ai studio-typing">"Thinking..."</div> }.into_view()
                            } else {
                                view! { <span></span> }.into_view()
                            }
                        }}
                    </div>
                    <div class="studio-input">
                        <textarea
                            class="studio-textarea"
                            placeholder="Tell the soul what to build..."
                            prop:value=move || input.get()
                            on:input=move |ev| set_input.set(event_target_value(&ev))
                            on:keydown=on_keydown
                            rows="3"
                        />
                        <button
                            class="studio-send"
                            on:click=move |_| send_message()
                            disabled=move || sending.get()
                        >
                            {move || if sending.get() { "Sending..." } else { "Send" }}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    }
}

/// Fetch file tree from the soul's workspace via list_directory tool.
async fn fetch_file_tree(path: &str) -> Result<Vec<FileEntry>, String> {
    // Use the soul chat to list files — the soul has list_directory tool
    // For now, use the diagnostics endpoint or a direct API
    let resp = gloo_net::http::Request::get(&format!("/soul/admin/ls?path={}", path))
        .send()
        .await
        .map_err(|e| format!("Failed: {e}"))?;

    if !resp.ok() {
        // Fallback: return empty
        return Ok(vec![]);
    }

    resp.json::<Vec<FileEntry>>()
        .await
        .map_err(|e| format!("Parse error: {e}"))
}

/// Fetch file content via the soul's read_file capability.
async fn fetch_file_content(path: &str) -> Result<String, String> {
    let resp = gloo_net::http::Request::get(&format!("/soul/admin/cat?path={}", path))
        .send()
        .await
        .map_err(|e| format!("Failed: {e}"))?;

    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }

    resp.text().await.map_err(|e| format!("Read error: {e}"))
}
