//! Cartridge browser — list, inspect, and test WASM cartridges.

use leptos::*;

use crate::api;

/// Cartridge browser page.
#[component]
pub fn CartridgesPage() -> impl IntoView {
    let (cartridges, set_cartridges) = create_signal(Vec::<serde_json::Value>::new());
    let (loading, set_loading) = create_signal(true);
    let (error, set_error) = create_signal(Option::<String>::None);
    let (test_result, set_test_result) = create_signal(Option::<String>::None);
    let (test_slug, set_test_slug) = create_signal(String::new());
    let (test_body, set_test_body) = create_signal(String::new());

    // Fetch cartridges on mount
    spawn_local(async move {
        match api::fetch_json("/c").await {
            Ok(data) => {
                if let Some(arr) = data.get("cartridges").and_then(|v| v.as_array()) {
                    set_cartridges.set(arr.clone());
                }
                set_loading.set(false);
            }
            Err(e) => {
                set_error.set(Some(e));
                set_loading.set(false);
            }
        }
    });

    let run_test = move |_| {
        let slug = test_slug.get();
        let body = test_body.get();
        if slug.is_empty() {
            return;
        }
        set_test_result.set(Some("Testing...".to_string()));
        spawn_local(async move {
            let url = format!("/c/{}", slug);
            match api::fetch_json(&url).await {
                Ok(data) => {
                    set_test_result.set(Some(
                        serde_json::to_string_pretty(&data).unwrap_or_default(),
                    ));
                }
                Err(e) => {
                    set_test_result.set(Some(format!("Error: {e}")));
                }
            }
        });
    };

    view! {
        <div class="page cartridges-page">
            <h1>"WASM Cartridges"</h1>
            <p class="subtitle">"Rust programs compiled to WASM — instant deployment, sandboxed execution"</p>

            <div class="cartridges-layout">
                // Left: cartridge list
                <div class="cartridges-list">
                    <h2>"Registered Cartridges"</h2>
                    {move || {
                        if loading.get() {
                            view! { <p>"Loading..."</p> }.into_view()
                        } else if let Some(err) = error.get() {
                            view! { <p class="error">{err}</p> }.into_view()
                        } else if cartridges.get().is_empty() {
                            view! {
                                <div class="empty-state">
                                    <p>"No cartridges yet."</p>
                                    <p>"Use the soul chat to create one:"</p>
                                    <code>"Create a hello-world cartridge in Rust"</code>
                                </div>
                            }.into_view()
                        } else {
                            view! {
                                <ul class="cartridge-entries">
                                    {cartridges.get().iter().map(|c| {
                                        let slug = c.get("slug").and_then(|v| v.as_str()).unwrap_or("?").to_string();
                                        let name = c.get("name").and_then(|v| v.as_str()).unwrap_or(&slug).to_string();
                                        let desc = c.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let price = c.get("price").and_then(|v| v.as_str()).unwrap_or("$0.001").to_string();
                                        let version = c.get("version").and_then(|v| v.as_str()).unwrap_or("0.1.0").to_string();
                                        let slug_for_test = slug.clone();
                                        view! {
                                            <li class="cartridge-entry" on:click=move |_| set_test_slug.set(slug_for_test.clone())>
                                                <div class="cartridge-name">{name}</div>
                                                <div class="cartridge-meta">
                                                    <span class="cartridge-slug">"/c/"{slug}</span>
                                                    <span class="cartridge-version">"v"{version}</span>
                                                    <span class="cartridge-price">{price}</span>
                                                </div>
                                                {(!desc.is_empty()).then(|| view! {
                                                    <div class="cartridge-desc">{desc}</div>
                                                })}
                                            </li>
                                        }
                                    }).collect::<Vec<_>>()}
                                </ul>
                            }.into_view()
                        }
                    }}
                </div>

                // Right: test console
                <div class="cartridges-test">
                    <h2>"Test Console"</h2>
                    <div class="test-form">
                        <label>"Slug"</label>
                        <input
                            type="text"
                            placeholder="hello-world"
                            prop:value=move || test_slug.get()
                            on:input=move |ev| set_test_slug.set(event_target_value(&ev))
                        />
                        <label>"Body (optional)"</label>
                        <textarea
                            placeholder="{\"key\": \"value\"}"
                            prop:value=move || test_body.get()
                            on:input=move |ev| set_test_body.set(event_target_value(&ev))
                        />
                        <button on:click=run_test>"Run Test"</button>
                    </div>
                    {move || test_result.get().map(|r| view! {
                        <div class="test-output">
                            <h3>"Response"</h3>
                            <pre>{r}</pre>
                        </div>
                    })}
                </div>
            </div>
        </div>
    }
}
