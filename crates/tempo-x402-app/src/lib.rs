use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

mod api;
mod wallet;
mod wallet_crypto;

/// Payment requirements from 402 response
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaymentRequirements {
    pub scheme: String,
    pub network: String,
    pub price: String,
    pub asset: String,
    pub amount: String,
    #[serde(rename = "payTo")]
    pub pay_to: String,
    #[serde(rename = "maxTimeoutSeconds")]
    pub max_timeout_seconds: u64,
    pub description: Option<String>,
}

/// Settlement response from server
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SettleResponse {
    pub success: bool,
    pub transaction: Option<String>,
    pub network: String,
    pub payer: Option<String>,
}

/// Wallet connection mode
#[derive(Clone, Debug, Default, PartialEq)]
pub enum WalletMode {
    #[default]
    Disconnected,
    MetaMask,
    DemoKey,
    Embedded,
}

impl WalletMode {
    fn label(&self) -> &'static str {
        match self {
            WalletMode::Disconnected => "",
            WalletMode::MetaMask => "MetaMask",
            WalletMode::DemoKey => "Demo",
            WalletMode::Embedded => "Embedded",
        }
    }
}

/// App state for wallet connection
#[derive(Clone, Debug, Default)]
pub struct WalletState {
    pub connected: bool,
    pub address: Option<String>,
    pub chain_id: Option<String>,
    pub mode: WalletMode,
    pub private_key: Option<String>,
}

/// Main application component
#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    let (wallet, set_wallet) = create_signal(WalletState::default());
    provide_context((wallet, set_wallet));

    view! {
        <Html lang="en" />
        <Meta charset="utf-8" />
        <Meta name="viewport" content="width=device-width, initial-scale=1" />
        <Title text="x402 Demo - Pay-per-Request APIs" />
        <Stylesheet href="/style.css" />

        <Router>
            <main class="container">
                <Header />
                <Routes>
                    <Route path="/" view=HomePage />
                    <Route path="/docs" view=DocsPage />
                    <Route path="/*any" view=NotFound />
                </Routes>
                <Footer />
            </main>
        </Router>
    }
}

/// Header with navigation and wallet connection
#[component]
fn Header() -> impl IntoView {
    let (wallet, set_wallet) =
        expect_context::<(ReadSignal<WalletState>, WriteSignal<WalletState>)>();

    view! {
        <header class="header">
            <nav class="nav">
                <a href="/" class="logo">"x402"</a>
                <div class="nav-links">
                    <a href="/">"Demo"</a>
                    <a href="/docs">"Docs"</a>
                    <a href="https://github.com/compusophy/tempo-x402" target="_blank">"GitHub"</a>
                </div>
                <WalletButtons wallet=wallet set_wallet=set_wallet />
            </nav>
        </header>
    }
}

/// Wallet connect/disconnect buttons with three modes
#[component]
fn WalletButtons(
    wallet: ReadSignal<WalletState>,
    set_wallet: WriteSignal<WalletState>,
) -> impl IntoView {
    let (funding, set_funding) = create_signal(false);

    let connect_metamask = move |_| {
        spawn_local(async move {
            match wallet::connect_wallet().await {
                Ok(state) => set_wallet.set(state),
                Err(e) => {
                    web_sys::console::error_1(&format!("MetaMask error: {}", e).into());
                }
            }
        });
    };

    let use_demo = move |_| match wallet::use_demo_key() {
        Ok(state) => set_wallet.set(state),
        Err(e) => {
            web_sys::console::error_1(&format!("Demo key error: {}", e).into());
        }
    };

    let create_wallet = move |_| {
        set_funding.set(true);
        match wallet::load_or_create_embedded_wallet() {
            Ok((state, is_new)) => {
                let address = state.address.clone().unwrap_or_default();
                set_wallet.set(state);
                if is_new {
                    // Only fund brand-new wallets
                    spawn_local(async move {
                        match wallet::fund_address(&address).await {
                            Ok(_) => {
                                web_sys::console::log_1(
                                    &format!("Funded new wallet: {}", address).into(),
                                );
                            }
                            Err(e) => {
                                web_sys::console::error_1(&format!("Funding failed: {}", e).into());
                            }
                        }
                        set_funding.set(false);
                    });
                } else {
                    web_sys::console::log_1(&format!("Restored wallet: {}", address).into());
                    set_funding.set(false);
                }
            }
            Err(e) => {
                web_sys::console::error_1(&format!("Create wallet error: {}", e).into());
                set_funding.set(false);
            }
        }
    };

    let disconnect = move |_| {
        // Disconnect only clears session state — the stored key remains in localStorage.
        // Users must explicitly "Delete Wallet" to remove the key.
        set_wallet.set(WalletState::default());
    };

    view! {
        <Show
            when=move || wallet.get().connected
            fallback=move || view! {
                <div class="wallet-buttons">
                    <button class="btn btn-primary" on:click=connect_metamask>
                        "Connect Wallet"
                    </button>
                    <button class="btn btn-secondary" on:click=use_demo>
                        "Demo Key"
                    </button>
                    <button
                        class="btn btn-secondary"
                        on:click=create_wallet
                        disabled=move || funding.get()
                    >
                        {move || {
                            if funding.get() {
                                "Creating..."
                            } else if wallet::has_stored_wallet() {
                                "Restore Wallet"
                            } else {
                                "Create Wallet"
                            }
                        }}
                    </button>
                </div>
            }
        >
            {move || {
                let w = wallet.get();
                let addr = w.address.unwrap_or_default();
                let short = if addr.len() > 10 {
                    format!("{}...{}", &addr[..6], &addr[addr.len()-4..])
                } else {
                    addr
                };
                let mode_label = w.mode.label();
                view! {
                    <div class="wallet-info">
                        <span class="wallet-mode-badge">{mode_label}</span>
                        <span class="wallet-address">{short}</span>
                        <button class="btn btn-secondary" on:click=disconnect>
                            "Disconnect"
                        </button>
                    </div>
                }
            }}
        </Show>
    }
}

/// Wallet management panel — export, import, delete for embedded wallets.
#[component]
fn WalletManagement() -> impl IntoView {
    let (wallet, set_wallet) =
        expect_context::<(ReadSignal<WalletState>, WriteSignal<WalletState>)>();

    let (show_key, set_show_key) = create_signal(false);
    let (show_import, set_show_import) = create_signal(false);
    let (import_value, set_import_value) = create_signal(String::new());
    let (import_error, set_import_error) = create_signal(None::<String>);
    let (confirm_delete, set_confirm_delete) = create_signal(false);
    let (copied, set_copied) = create_signal(false);

    let toggle_reveal = move |_| {
        set_show_key.update(|v| *v = !*v);
        set_copied.set(false);
    };

    let copy_key = move |_| {
        if let Some(key) = wallet.get().private_key {
            wallet::copy_to_clipboard(&key);
            set_copied.set(true);
        }
    };

    let download_json = move |_| {
        let w = wallet.get();
        if let (Some(key), Some(addr)) = (w.private_key, w.address) {
            let json = wallet::export_wallet_json(&key, &addr);
            let filename = format!("x402-wallet-{}.json", &addr[..8]);
            if let Err(e) = wallet::trigger_download(&filename, &json) {
                web_sys::console::error_1(&format!("Download failed: {}", e).into());
            }
        }
    };

    let toggle_import = move |_| {
        set_show_import.update(|v| *v = !*v);
        set_import_error.set(None);
        set_import_value.set(String::new());
    };

    let do_import = move |_| {
        let key = import_value.get();
        match wallet::import_embedded_wallet(&key) {
            Ok(state) => {
                set_wallet.set(state);
                set_show_import.set(false);
                set_import_error.set(None);
            }
            Err(e) => {
                set_import_error.set(Some(e));
            }
        }
    };

    let do_delete = move |_| {
        wallet::delete_embedded_wallet();
        set_wallet.set(WalletState::default());
        set_confirm_delete.set(false);
    };

    view! {
        <Show when=move || wallet.get().mode == WalletMode::Embedded fallback=|| ()>
            <div class="wallet-management">
                <h4>"Wallet Management"</h4>

                <div class="wallet-actions">
                    // --- Reveal / Copy ---
                    <button class="btn btn-secondary btn-sm" on:click=toggle_reveal>
                        {move || if show_key.get() { "Hide Key" } else { "Reveal Key" }}
                    </button>

                    <Show when=move || show_key.get() fallback=|| ()>
                        <div class="key-reveal">
                            <code class="private-key">{move || wallet.get().private_key.unwrap_or_default()}</code>
                            <button class="btn btn-secondary btn-sm" on:click=copy_key>
                                {move || if copied.get() { "Copied!" } else { "Copy" }}
                            </button>
                        </div>
                    </Show>

                    // --- Download JSON ---
                    <button class="btn btn-secondary btn-sm" on:click=download_json>
                        "Download Backup"
                    </button>

                    // --- Import ---
                    <button class="btn btn-secondary btn-sm" on:click=toggle_import>
                        {move || if show_import.get() { "Cancel Import" } else { "Import Key" }}
                    </button>

                    <Show when=move || show_import.get() fallback=|| ()>
                        <div class="import-form">
                            <input
                                type="password"
                                class="input"
                                placeholder="Paste private key (0x...)"
                                prop:value=move || import_value.get()
                                on:input=move |ev| {
                                    set_import_value.set(event_target_value(&ev));
                                    set_import_error.set(None);
                                }
                            />
                            <button class="btn btn-primary btn-sm" on:click=do_import>
                                "Import"
                            </button>
                            <Show when=move || import_error.get().is_some() fallback=|| ()>
                                <p class="error-text">{move || import_error.get().unwrap_or_default()}</p>
                            </Show>
                        </div>
                    </Show>

                    // --- Delete (destructive) ---
                    <Show
                        when=move || confirm_delete.get()
                        fallback=move || view! {
                            <button
                                class="btn btn-danger btn-sm"
                                on:click=move |_| set_confirm_delete.set(true)
                            >
                                "Delete Wallet"
                            </button>
                        }
                    >
                        <div class="delete-confirm">
                            <p class="warning-text">
                                "This permanently deletes your private key. Make sure you have a backup!"
                            </p>
                            <button class="btn btn-danger btn-sm" on:click=do_delete>
                                "Yes, Delete Forever"
                            </button>
                            <button
                                class="btn btn-secondary btn-sm"
                                on:click=move |_| set_confirm_delete.set(false)
                            >
                                "Cancel"
                            </button>
                        </div>
                    </Show>
                </div>
            </div>
        </Show>
    }
}

/// Instance info panel — shows identity, children, clone button
#[component]
fn InstancePanel() -> impl IntoView {
    let (info, set_info) = create_signal(None::<serde_json::Value>);
    let (loading, set_loading) = create_signal(true);
    let (error, set_error) = create_signal(None::<String>);

    // Fetch instance info on mount
    spawn_local(async move {
        let base = api::gateway_base_url();
        let url = format!("{}/instance/info", base);
        match gloo_net::http::Request::get(&url).send().await {
            Ok(resp) if resp.ok() => {
                if let Ok(data) = resp.json::<serde_json::Value>().await {
                    set_info.set(Some(data));
                }
            }
            Ok(resp) => {
                set_error.set(Some(format!("HTTP {}", resp.status())));
            }
            Err(e) => {
                set_error.set(Some(format!("{}", e)));
            }
        }
        set_loading.set(false);
    });

    view! {
        <div class="instance-panel">
            <h3>"Instance Info"</h3>

            <Show when=move || loading.get() fallback=|| ()>
                <p class="loading">"Loading instance info..."</p>
            </Show>

            <Show when=move || error.get().is_some() fallback=|| ()>
                <p class="error-text">"Instance info unavailable"</p>
            </Show>

            <Show when=move || info.get().is_some() fallback=|| ()>
                {move || {
                    let data = info.get().unwrap_or_default();

                    let identity = data.get("identity").cloned();
                    let children_count = data.get("children_count")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    let clone_available = data.get("clone_available")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let clone_price = data.get("clone_price")
                        .and_then(|v| v.as_str())
                        .unwrap_or("N/A")
                        .to_string();
                    let version = data.get("version")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let uptime = data.get("uptime_seconds")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);

                    let address = identity.as_ref()
                        .and_then(|id| id.get("address"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("N/A")
                        .to_string();
                    let instance_id = identity.as_ref()
                        .and_then(|id| id.get("instance_id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("N/A")
                        .to_string();
                    let parent_url = identity.as_ref()
                        .and_then(|id| id.get("parent_url"))
                        .and_then(|v| v.as_str())
                        .map(String::from);

                    let children = data.get("children")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();

                    view! {
                        <div class="instance-details">
                            <div class="instance-identity">
                                <p><strong>"Address: "</strong><code>{address}</code></p>
                                <p><strong>"Instance: "</strong><code>{instance_id}</code></p>
                                <p><strong>"Version: "</strong>{version}</p>
                                <p><strong>"Uptime: "</strong>{format!("{}s", uptime)}</p>
                                {parent_url.map(|url| view! {
                                    <p><strong>"Parent: "</strong>
                                        <a href=url.clone() target="_blank">{url}</a>
                                    </p>
                                })}
                            </div>

                            <Show when=move || clone_available fallback=|| ()>
                                <div class="clone-section">
                                    <p>"Clone this instance for " <strong>{clone_price.clone()}</strong></p>
                                </div>
                            </Show>

                            <Show when=move || { children_count > 0 } fallback=|| ()>
                                <div class="children-list">
                                    <h4>{format!("Children ({})", children_count)}</h4>
                                    <ul>
                                        {children.iter().map(|child| {
                                            let child_id = child.get("instance_id")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("unknown")
                                                .to_string();
                                            let child_url = child.get("url")
                                                .and_then(|v| v.as_str())
                                                .map(String::from);
                                            let child_status = child.get("status")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("unknown")
                                                .to_string();
                                            view! {
                                                <li>
                                                    <code>{child_id}</code>
                                                    " — "
                                                    <span class="status-badge">{child_status}</span>
                                                    {child_url.map(|url| view! {
                                                        " "
                                                        <a href=url.clone() target="_blank">{url}</a>
                                                    })}
                                                </li>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </ul>
                                </div>
                            </Show>
                        </div>
                    }
                }}
            </Show>
        </div>
    }
}

/// Home page with payment demo
#[component]
fn HomePage() -> impl IntoView {
    view! {
        <div class="page">
            <h1>"x402 Payment Demo"</h1>
            <p class="subtitle">
                "HTTP 402 Payment Required — pay-per-request APIs on Tempo"
            </p>

            <PaymentDemo />
            <WalletManagement />
            <InstancePanel />

            <div class="info-section">
                <h2>"How it works"</h2>
                <ol class="steps">
                    <li>"Connect a wallet (MetaMask, demo key, or create an embedded wallet)"</li>
                    <li>"Click \"Pay & Request\" to hit a paid API endpoint"</li>
                    <li>"The gateway returns 402 with payment requirements"</li>
                    <li>"Your wallet signs an EIP-712 payment authorization"</li>
                    <li>"The request retries with PAYMENT-SIGNATURE header"</li>
                    <li>"The facilitator settles the payment on-chain"</li>
                    <li>"You get the API response + a transaction hash"</li>
                </ol>
            </div>
        </div>
    }
}

/// Payment demo component
#[component]
fn PaymentDemo() -> impl IntoView {
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

/// Documentation page
#[component]
fn DocsPage() -> impl IntoView {
    view! {
                <div class="page docs">
                    <h1>"Documentation"</h1>

                    <section>
                        <h2>"Quick Start"</h2>
                        <pre class="code-block">
    {r#"// Add to Cargo.toml
[dependencies]
tempo-x402-client = "0.4"

// Make paid requests
use x402_client::{X402Client, TempoSchemeClient};

let signer = "0x...".parse().unwrap();
let client = X402Client::new(TempoSchemeClient::new(signer));

let (resp, settlement) = client
    .fetch("https://api.example.com/data", Method::GET)
    .await?;
"#}
                        </pre>
                    </section>

                    <section>
                        <h2>"Crates"</h2>
                        <ul>
                            <li><a href="https://crates.io/crates/tempo-x402">"tempo-x402"</a>" — Core types and crypto"</li>
                            <li><a href="https://crates.io/crates/tempo-x402-client">"tempo-x402-client"</a>" — Client SDK"</li>
                            <li><a href="https://crates.io/crates/tempo-x402-server">"tempo-x402-server"</a>" — Server middleware"</li>
                            <li><a href="https://crates.io/crates/tempo-x402-facilitator">"tempo-x402-facilitator"</a>" — Payment settlement"</li>
                            <li><a href="https://crates.io/crates/tempo-x402-gateway">"tempo-x402-gateway"</a>" — API gateway"</li>
                            <li><a href="https://crates.io/crates/tempo-x402-wallet">"tempo-x402-wallet"</a>" — WASM wallet (signing + key gen)"</li>
                        </ul>
                    </section>

                    <section>
                        <h2>"Links"</h2>
                        <ul>
                            <li><a href="https://github.com/compusophy/tempo-x402">"GitHub"</a></li>
                            <li><a href="https://explore.moderato.tempo.xyz">"Block Explorer"</a></li>
                        </ul>
                    </section>
                </div>
            }
}

/// Footer
#[component]
fn Footer() -> impl IntoView {
    view! {
        <footer class="footer">
            <p>
                "Built with "
                <a href="https://github.com/compusophy/tempo-x402">"tempo-x402"</a>
                " on Tempo Moderato"
            </p>
        </footer>
    }
}

/// 404 page
#[component]
fn NotFound() -> impl IntoView {
    view! {
        <div class="page">
            <h1>"404 - Not Found"</h1>
            <p><a href="/">"Go home"</a></p>
        </div>
    }
}

/// Initialize the app
#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(|| view! { <App /> });
}
