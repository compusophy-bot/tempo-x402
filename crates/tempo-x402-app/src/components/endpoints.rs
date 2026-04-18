use crate::{api, WalletState};
use leptos::*;

/// Endpoint registration form component
#[component]
pub fn EndpointRegistration() -> impl IntoView {
    let (wallet, _) = expect_context::<(ReadSignal<WalletState>, WriteSignal<WalletState>)>();

    let (slug, set_slug) = create_signal(String::new());
    let (target_url, set_target_url) = create_signal(String::new());
    let (price, set_price) = create_signal(String::from("0.001"));
    let (_description, set_description) = create_signal(String::new());
    let (loading, set_loading) = create_signal(false);
    let (error, set_error) = create_signal(None::<String>);
    let (success, set_success) = create_signal(None::<String>);

    let slug_valid = move || {
        let s = slug.get();
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    };

    let url_valid = move || {
        let u = target_url.get();
        u.starts_with("https://") || u.starts_with("http://")
    };

    let price_valid = move || {
        let p = price.get();
        p.parse::<f64>().map(|v| v > 0.0).unwrap_or(false)
    };

    let can_submit = move || {
        wallet.get().connected && slug_valid() && url_valid() && price_valid() && !loading.get()
    };

    let do_register = move |_| {
        if !can_submit() {
            return;
        }

        set_loading.set(true);
        set_error.set(None);
        set_success.set(None);

        let s = slug.get();
        let u = target_url.get();
        let p = price.get();

        spawn_local(async move {
            match api::register_endpoint(&s, &u, &p).await {
                Ok(resp) => {
                    let gw_url = resp
                        .get("gateway_url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    set_success.set(Some(format!(
                        "Registered /g/{} -> {} (gateway: {})",
                        s, u, gw_url
                    )));
                    set_slug.set(String::new());
                    set_target_url.set(String::new());
                    set_price.set("0.001".to_string());
                    set_description.set(String::new());
                }
                Err(e) => {
                    set_error.set(Some(e));
                }
            }
            set_loading.set(false);
        });
    };

    view! {
        <div class="registration-card">
            <h3>"Register an Endpoint"</h3>
            <p class="registration-subtitle">
                "Expose any URL behind a paywall — clients pay per request via the x402 protocol"
            </p>

            <div class="registration-form">
                <div class="form-group">
                    <label>"Slug"</label>
                    <input
                        type="text"
                        placeholder="my-api"
                        prop:value=move || slug.get()
                        on:input=move |ev| set_slug.set(event_target_value(&ev))
                    />
                    {move || {
                        let s = slug.get();
                        if !s.is_empty() && !slug_valid() {
                            view! { <p class="input-error">"Only alphanumeric, hyphens, underscores"</p> }.into_view()
                        } else if !s.is_empty() {
                            view! { <p class="input-hint">{format!("Access via /g/{}", s)}</p> }.into_view()
                        } else {
                            view! { <p class="input-hint">"URL-safe identifier"</p> }.into_view()
                        }
                    }}
                </div>
                <div class="form-group">
                    <label>"Target URL"</label>
                    <input
                        type="text"
                        placeholder="https://api.example.com"
                        prop:value=move || target_url.get()
                        on:input=move |ev| set_target_url.set(event_target_value(&ev))
                    />
                    {move || {
                        let u = target_url.get();
                        if !u.is_empty() && !url_valid() {
                            view! { <p class="input-error">"Must start with https:// or http://"</p> }.into_view()
                        } else {
                            view! { <p class="input-hint">"Upstream URL to proxy requests to"</p> }.into_view()
                        }
                    }}
                </div>
                <div class="form-group">
                    <label>"Price (USD)"</label>
                    <input
                        type="text"
                        placeholder="0.001"
                        prop:value=move || price.get()
                        on:input=move |ev| set_price.set(event_target_value(&ev))
                    />
                    {move || {
                        if !price_valid() && !price.get().is_empty() {
                            view! { <p class="input-error">"Enter a positive number"</p> }.into_view()
                        } else {
                            view! { <p class="input-hint">"Per-request cost in pathUSD"</p> }.into_view()
                        }
                    }}
                </div>
            </div>

            <div class="registration-actions">
                <button
                    class="btn btn-primary"
                    on:click=do_register
                    disabled=move || !can_submit()
                >
                    {move || if loading.get() { "Registering..." } else { "Register Endpoint" }}
                </button>
                {move || {
                    if !wallet.get().connected {
                        view! { <span class="soul-muted">"Connect wallet to register"</span> }.into_view()
                    } else {
                        view! { <span></span> }.into_view()
                    }
                }}
            </div>

            <Show when=move || error.get().is_some() fallback=|| ()>
                <p class="error-text" style="margin-top: 12px">
                    {move || error.get().unwrap_or_default()}
                </p>
            </Show>

            <Show when=move || success.get().is_some() fallback=|| ()>
                <div class="registration-success" style="margin-top: 12px">
                    {move || success.get().unwrap_or_default()}
                </div>
            </Show>
        </div>
    }
}
