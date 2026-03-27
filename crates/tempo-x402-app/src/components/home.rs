use leptos::*;

use super::instance::InstancePanel;
use super::wallet_panel::WalletManagement;
use crate::api;
use crate::*;

#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <div class="page">
            <h1>"x402 Node"</h1>
            <p class="subtitle">
                "Self-replicating AI agents that pay each other with crypto"
            </p>

            <WalletManagement />
            <InstancePanel />
        </div>
    }
}

/// Payment demo component
#[component]
pub fn PaymentDemo() -> impl IntoView {
    let (wallet, _) = expect_context::<(ReadSignal<WalletState>, WriteSignal<WalletState>)>();
    let (status, set_status) = create_signal(String::from(
        "Ready — connect a wallet and click Pay & Request",
    ));
    let (result, set_result) = create_signal(None::<String>);
    let (tx_hash, set_tx_hash) = create_signal(None::<String>);
    let (loading, set_loading) = create_signal(false);

    let make_request = move |_| {
        let w = wallet.get();
        if !w.connected {
            set_status
                .set("Connect a wallet first (MetaMask, Demo Key, or Create Wallet)".to_string());
            return;
        }

        set_loading.set(true);
        set_status.set("Requesting /g/demo — expecting 402...".to_string());
        set_result.set(None);
        set_tx_hash.set(None);

        spawn_local(async move {
            match api::make_paid_request(&w).await {
                Ok((data, settle)) => {
                    if let Some(ref s) = settle {
                        if s.success {
                            set_status.set("Payment settled on-chain!".to_string());
                        } else {
                            set_status.set("Payment sent (check transaction)".to_string());
                        }
                        set_tx_hash.set(s.transaction.clone());
                    } else {
                        set_status.set("Response received (no settlement header)".to_string());
                    }
                    set_result.set(Some(data));
                }
                Err(e) => {
                    set_status.set(format!("Error: {}", e));
                }
            }
            set_loading.set(false);
        });
    };

    view! {
        <div class="demo-card">
            <h3>"Make a Paid Request"</h3>
            <p class="demo-description">
                "Calls " <code>"/g/demo"</code> " — a paid proxy to httpbin.org ($0.001 per request)"
            </p>

            <div class="demo-controls">
                <button
                    class="btn btn-primary"
                    on:click=make_request
                    disabled=move || loading.get()
                >
                    {move || if loading.get() { "Signing & paying..." } else { "Pay & Request ($0.001)" }}
                </button>
            </div>

            <div class="demo-status">
                <p class="status-text">{move || status.get()}</p>
            </div>

            <Show when=move || tx_hash.get().is_some() fallback=|| ()>
                <div class="demo-tx">
                    <h4>"Transaction"</h4>
                    <a
                        href=move || format!("https://explore.moderato.tempo.xyz/tx/{}", tx_hash.get().unwrap_or_default())
                        target="_blank"
                        class="tx-link"
                    >
                        {move || tx_hash.get().unwrap_or_default()}
                    </a>
                </div>
            </Show>

            <Show when=move || result.get().is_some() fallback=|| ()>
                <div class="demo-result">
                    <h4>"Proxied Response"</h4>
                    <pre class="code-block">{move || result.get().unwrap_or_default()}</pre>
                </div>
            </Show>
        </div>
    }
}

/// Format seconds into human-readable uptime string
fn format_uptime(secs: i64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d {}h", secs / 86400, (secs % 86400) / 3600)
    }
}
