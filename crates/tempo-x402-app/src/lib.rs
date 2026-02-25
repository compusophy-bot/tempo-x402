use gloo_timers::callback::Interval;
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
                    <Route path="/dashboard" view=DashboardPage />
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
                    <a href="/dashboard">"Dashboard"</a>
                    <a href="https://docs.rs/tempo-x402" target="_blank">"Docs"</a>
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

/// Shorten an address for display
fn shorten_address(addr: &str) -> String {
    if addr.len() > 12 {
        format!("{}...{}", &addr[..6], &addr[addr.len() - 4..])
    } else {
        addr.to_string()
    }
}

/// Dashboard page with live network topology
#[component]
fn DashboardPage() -> impl IntoView {
    let (info, set_info) = create_signal(None::<serde_json::Value>);
    let (endpoints, set_endpoints) = create_signal(Vec::<serde_json::Value>::new());
    let (analytics, set_analytics) = create_signal(None::<serde_json::Value>);
    let (child_health, set_child_health) =
        create_signal(std::collections::HashMap::<String, serde_json::Value>::new());
    let (soul_status, set_soul_status) = create_signal(None::<serde_json::Value>);
    let (loading, set_loading) = create_signal(true);
    let (error, set_error) = create_signal(None::<String>);
    let (tick, set_tick) = create_signal(0u32);

    // Fetch all dashboard data
    let fetch_data = move || {
        spawn_local(async move {
            let base = api::gateway_base_url();

            // Fetch instance info
            match gloo_net::http::Request::get(&format!("{}/instance/info", base))
                .send()
                .await
            {
                Ok(resp) if resp.ok() => {
                    if let Ok(data) = resp.json::<serde_json::Value>().await {
                        // Fetch child health for each child with a URL
                        if let Some(children) = data.get("children").and_then(|v| v.as_array()) {
                            for child in children {
                                if let Some(url) = child.get("url").and_then(|v| v.as_str()) {
                                    let url = url.to_string();
                                    let child_id = child
                                        .get("instance_id")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown")
                                        .to_string();
                                    spawn_local(async move {
                                        if let Ok(resp) =
                                            gloo_net::http::Request::get(&format!("{}/health", url))
                                                .send()
                                                .await
                                        {
                                            if let Ok(health) =
                                                resp.json::<serde_json::Value>().await
                                            {
                                                set_child_health.update(|map| {
                                                    map.insert(child_id, health);
                                                });
                                            }
                                        }
                                    });
                                }
                            }
                        }
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

            // Fetch endpoints
            if let Ok(eps) = api::list_endpoints().await {
                set_endpoints.set(eps);
            }

            // Fetch analytics
            if let Ok(data) = api::fetch_analytics().await {
                set_analytics.set(Some(data));
            }

            // Fetch soul status
            if let Ok(data) = api::fetch_soul_status().await {
                set_soul_status.set(Some(data));
            }

            set_loading.set(false);
        });
    };

    // Initial fetch
    fetch_data();

    // Auto-refresh every 10s
    let interval = Interval::new(10_000, move || {
        set_tick.update(|t| *t = t.wrapping_add(1));
        fetch_data();
    });

    on_cleanup(move || {
        drop(interval);
    });

    view! {
        <div class="page dashboard">
            <div class="dashboard-header">
                <h1>"Network Dashboard"</h1>
                <div class="live-badge">
                    <span class="live-dot"></span>
                    "Live"
                    {move || {
                        // Touch tick to ensure reactivity
                        let _ = tick.get();
                    }}
                </div>
            </div>

            <Show when=move || loading.get() && info.get().is_none() fallback=|| ()>
                <p class="loading">"Loading dashboard..."</p>
            </Show>

            <Show when=move || error.get().is_some() && info.get().is_none() fallback=|| ()>
                <p class="error-text">{move || error.get().unwrap_or_default()}</p>
            </Show>

            <Show when=move || info.get().is_some() fallback=|| ()>
                {move || {
                    let data = info.get().unwrap_or_default();

                    let version = data.get("version")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let uptime = data.get("uptime_seconds")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    let children_count = data.get("children_count")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    let max_children = data.get("clone_max_children")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(10);
                    let clone_available = data.get("clone_available")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let clone_price = data.get("clone_price")
                        .and_then(|v| v.as_str())
                        .unwrap_or("N/A")
                        .to_string();

                    let identity = data.get("identity").cloned();
                    let address = identity.as_ref()
                        .and_then(|id| id.get("address"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("N/A")
                        .to_string();
                    let parent_url = identity.as_ref()
                        .and_then(|id| id.get("parent_url"))
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let parent_address = identity.as_ref()
                        .and_then(|id| id.get("parent_address"))
                        .and_then(|v| v.as_str())
                        .map(String::from);

                    let children = data.get("children")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();

                    let health_map = child_health.get();
                    let eps = endpoints.get();
                    let ep_count = eps.len();

                    let analytics_data = analytics.get();
                    let total_payments = analytics_data.as_ref()
                        .and_then(|a| a.get("total_payments"))
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    let total_revenue_usd = analytics_data.as_ref()
                        .and_then(|a| a.get("total_revenue_usd"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("$0")
                        .to_string();
                    let analytics_endpoints = analytics_data.as_ref()
                        .and_then(|a| a.get("endpoints"))
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    let active_endpoints = analytics_endpoints.len();

                    view! {
                        // Stats cards
                        <div class="stats-grid">
                            <div class="stat-card">
                                <span class="stat-label">"STATUS"</span>
                                <span class="stat-value">
                                    <span class="status-dot status-dot--green"></span>
                                    " Online"
                                </span>
                            </div>
                            <div class="stat-card">
                                <span class="stat-label">"VERSION"</span>
                                <span class="stat-value">{format!("v{}", version)}</span>
                            </div>
                            <div class="stat-card">
                                <span class="stat-label">"UPTIME"</span>
                                <span class="stat-value">{format_uptime(uptime)}</span>
                            </div>
                            <div class="stat-card">
                                <span class="stat-label">"CLONES"</span>
                                <span class="stat-value">
                                    {format!("{}/{}", children_count, max_children)}
                                    {if clone_available {
                                        " available"
                                    } else {
                                        ""
                                    }}
                                </span>
                            </div>
                        </div>

                        // Analytics stats cards
                        <div class="stats-grid">
                            <div class="stat-card">
                                <span class="stat-label">"TOTAL PAYMENTS"</span>
                                <span class="stat-value">{total_payments.to_string()}</span>
                            </div>
                            <div class="stat-card">
                                <span class="stat-label">"TOTAL REVENUE"</span>
                                <span class="stat-value">{total_revenue_usd}</span>
                            </div>
                            <div class="stat-card">
                                <span class="stat-label">"ACTIVE ENDPOINTS"</span>
                                <span class="stat-value">{active_endpoints.to_string()}</span>
                            </div>
                        </div>

                        // Soul panel
                        <SoulPanel status=soul_status />

                        // Network topology
                        <div class="topology-section">
                            <h2>"Network Topology"</h2>
                            <div class="topology">
                                // Parent node
                                <div class="topology-level">
                                    <div class="node-card node-card--parent">
                                        <div class="node-header">
                                            <span class="status-dot status-dot--green"></span>
                                            <strong>"Parent (this node)"</strong>
                                        </div>
                                        <code class="node-address">{shorten_address(&address)}</code>
                                        <div class="node-meta">
                                            <span>{format!("v{}", version)}</span>
                                            <span>{format!("↑ {}", format_uptime(uptime))}</span>
                                        </div>
                                        <div class="node-meta">
                                            <span>{format!("clone: {}  {}/{} slots", clone_price, children_count, max_children)}</span>
                                        </div>
                                        {parent_url.map(|url| {
                                            let parent_short = parent_address
                                                .as_deref()
                                                .map(shorten_address)
                                                .unwrap_or_else(|| "parent".to_string());
                                            view! {
                                                <div class="node-parent-link">
                                                    "↑ "
                                                    <a href=url target="_blank">{parent_short}</a>
                                                </div>
                                            }
                                        })}
                                    </div>
                                </div>

                                // Connector
                                {if !children.is_empty() {
                                    let children_view = children;
                                    Some(view! {
                                    <div class="connector-vertical"></div>
                                    <div class="topology-level topology-level--children">
                                        {children_view.iter().map(|child| {
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
                                            let child_address = child.get("address")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("")
                                                .to_string();

                                            let health = health_map.get(&child_id).cloned();

                                            let (dot_class, status_label) = match child_status.as_str() {
                                                "running" => ("status-dot--green", "running"),
                                                "deploying" | "building" => ("status-dot--yellow", &*child_status),
                                                "failed" | "error" => ("status-dot--red", &*child_status),
                                                _ => ("status-dot--yellow", &*child_status),
                                            };

                                            let child_version = health.as_ref()
                                                .and_then(|h| h.get("version"))
                                                .and_then(|v| v.as_str())
                                                .map(|v| format!("v{}", v))
                                                .unwrap_or_else(|| {
                                                    if child_status == "running" {
                                                        "loading...".to_string()
                                                    } else {
                                                        "v?.?.?".to_string()
                                                    }
                                                });

                                            let display_addr = if child_address.is_empty() {
                                                child_id.clone()
                                            } else {
                                                shorten_address(&child_address)
                                            };

                                            view! {
                                                <div class="node-card node-card--child">
                                                    <div class="node-header">
                                                        <span class={format!("status-dot {}", dot_class)}></span>
                                                        <strong>{status_label.to_string()}</strong>
                                                    </div>
                                                    {child_url.as_ref().map(|url| view! {
                                                        <a href=url.clone() target="_blank" class="node-address">
                                                            {display_addr.clone()}
                                                        </a>
                                                    })}
                                                    {if child_url.is_none() {
                                                        Some(view! {
                                                            <code class="node-address">{display_addr.clone()}</code>
                                                        })
                                                    } else {
                                                        None
                                                    }}
                                                    <div class="node-meta">
                                                        <span>{child_version}</span>
                                                    </div>
                                                </div>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </div>
                                    })
                                } else {
                                    None
                                }}
                            </div>
                        </div>

                        // Endpoints table
                        <div class="endpoints-section">
                            <h2>{format!("Registered Endpoints ({})", ep_count)}</h2>
                            {if eps.is_empty() {
                                view! { <p class="empty">"No endpoints registered"</p> }.into_view()
                            } else {
                                let analytics_eps = analytics_endpoints.clone();
                                view! {
                                    <div class="endpoints-table">
                                        <div class="endpoint-row endpoint-header">
                                            <span class="endpoint-slug">"Endpoint"</span>
                                            <span class="endpoint-price">"Price"</span>
                                            <span class="endpoint-stat">"Calls"</span>
                                            <span class="endpoint-stat">"Payments"</span>
                                            <span class="endpoint-stat">"Revenue"</span>
                                            <span class="endpoint-desc">"Description"</span>
                                        </div>
                                        {eps.iter().map(|ep| {
                                            let slug = ep.get("slug")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("?")
                                                .to_string();
                                            let price = ep.get("price")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("?")
                                                .to_string();
                                            let description = ep.get("description")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("")
                                                .to_string();
                                            let gateway_url = ep.get("gateway_url")
                                                .and_then(|v| v.as_str())
                                                .map(String::from);

                                            // Look up analytics stats for this slug
                                            let ep_stats = analytics_eps.iter().find(|a| {
                                                a.get("slug").and_then(|v| v.as_str()) == Some(&slug)
                                            });
                                            let calls = ep_stats
                                                .and_then(|s| s.get("request_count"))
                                                .and_then(|v| v.as_i64())
                                                .unwrap_or(0);
                                            let payments = ep_stats
                                                .and_then(|s| s.get("payment_count"))
                                                .and_then(|v| v.as_i64())
                                                .unwrap_or(0);
                                            let revenue = ep_stats
                                                .and_then(|s| s.get("revenue_usd"))
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("$0")
                                                .to_string();

                                            view! {
                                                <div class="endpoint-row">
                                                    <span class="endpoint-slug">
                                                        {gateway_url.as_ref().map(|url| view! {
                                                            <a href=url.clone() target="_blank">{format!("/g/{}", slug)}</a>
                                                        })}
                                                        {if gateway_url.is_none() {
                                                            Some(view! { <span>{format!("/g/{}", slug)}</span> })
                                                        } else {
                                                            None
                                                        }}
                                                    </span>
                                                    <span class="endpoint-price">{format!("${}", price)}</span>
                                                    <span class="endpoint-stat">{calls.to_string()}</span>
                                                    <span class="endpoint-stat">{payments.to_string()}</span>
                                                    <span class="endpoint-stat">{revenue}</span>
                                                    <span class="endpoint-desc">{description}</span>
                                                </div>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </div>
                                }.into_view()
                            }}
                        </div>
                    }
                }}
            </Show>
        </div>
    }
}

/// Format a unix timestamp as relative time (e.g., "2m ago")
fn format_relative_time(unix_ts: i64) -> String {
    let now = (js_sys::Date::now() / 1000.0) as i64;
    let diff = now - unix_ts;
    if diff < 0 {
        return "just now".to_string();
    }
    if diff < 60 {
        format!("{}s ago", diff)
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        format!("{}d ago", diff / 86400)
    }
}

/// Soul observability panel
#[component]
fn SoulPanel(status: ReadSignal<Option<serde_json::Value>>) -> impl IntoView {
    view! {
        <div class="soul-section">
            {move || {
                let data = match status.get() {
                    Some(d) => d,
                    None => return view! {
                        <div class="soul-card soul-card--inactive">
                            <div class="soul-header">
                                <h2>"Soul"</h2>
                                <span class="soul-status-badge soul-status--gray">"No Data"</span>
                            </div>
                            <p class="soul-muted">"Soul status unavailable"</p>
                        </div>
                    }.into_view(),
                };

                let active = data.get("active").and_then(|v| v.as_bool()).unwrap_or(false);
                let dormant = data.get("dormant").and_then(|v| v.as_bool()).unwrap_or(false);
                let total_cycles = data.get("total_cycles").and_then(|v| v.as_u64()).unwrap_or(0);
                let last_think_at = data.get("last_think_at").and_then(|v| v.as_i64());
                let thoughts = data.get("recent_thoughts")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();

                let (badge_class, badge_label) = if !active {
                    ("soul-status--gray", "Inactive")
                } else if dormant {
                    ("soul-status--yellow", "Dormant")
                } else {
                    ("soul-status--green", "Active")
                };

                let last_thought_str = last_think_at
                    .map(format_relative_time)
                    .unwrap_or_else(|| "never".to_string());

                let mode_label = if !active {
                    "Inactive"
                } else if dormant {
                    "Dormant"
                } else {
                    "Active"
                };

                view! {
                    <div class="soul-card">
                        <div class="soul-header">
                            <h2>"Soul"</h2>
                            <span class={format!("soul-status-badge {}", badge_class)}>
                                {badge_label}
                            </span>
                        </div>

                        <div class="stats-grid">
                            <div class="stat-card">
                                <span class="stat-label">"CYCLES"</span>
                                <span class="stat-value">{total_cycles.to_string()}</span>
                            </div>
                            <div class="stat-card">
                                <span class="stat-label">"LAST THOUGHT"</span>
                                <span class="stat-value">{last_thought_str}</span>
                            </div>
                            <div class="stat-card">
                                <span class="stat-label">"MODE"</span>
                                <span class="stat-value">{mode_label}</span>
                            </div>
                        </div>

                        {if thoughts.is_empty() && !active {
                            view! {
                                <p class="soul-muted">"Soul not active"</p>
                            }.into_view()
                        } else if thoughts.is_empty() {
                            view! {
                                <p class="soul-muted">"No thoughts recorded yet"</p>
                            }.into_view()
                        } else {
                            view! {
                                <div class="soul-thoughts">
                                    <h3>"Recent Thoughts"</h3>
                                    {thoughts.iter().map(|t| {
                                        let thought_type = t.get("type")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("unknown")
                                            .to_string();
                                        let content = t.get("content")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        let created_at = t.get("created_at")
                                            .and_then(|v| v.as_i64())
                                            .unwrap_or(0);

                                        let badge_abbr = match thought_type.as_str() {
                                            "observation" => "obs",
                                            "reasoning" => "reason",
                                            "decision" => "decide",
                                            "reflection" => "reflect",
                                            _ => &thought_type,
                                        };

                                        // Truncate long content
                                        let display_content = if content.len() > 120 {
                                            format!("{}...", &content[..120])
                                        } else {
                                            content
                                        };

                                        view! {
                                            <div class="soul-thought">
                                                <span class={format!("thought-badge thought-badge--{}", thought_type)}>
                                                    {badge_abbr.to_string()}
                                                </span>
                                                <span class="thought-content">{display_content}</span>
                                                <span class="thought-time">{format_relative_time(created_at)}</span>
                                            </div>
                                        }
                                    }).collect::<Vec<_>>()}
                                </div>
                            }.into_view()
                        }}
                    </div>
                }.into_view()
            }}
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
