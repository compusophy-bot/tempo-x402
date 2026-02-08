use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

mod api;
mod wallet;

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
                                web_sys::console::log_1(&format!("Funded new wallet: {}", address).into());
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
        // Clear persisted embedded wallet on disconnect
        if wallet.get().mode == WalletMode::Embedded {
            wallet::clear_embedded_wallet();
        }
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
                        {move || if funding.get() { "Creating..." } else { "Embedded Wallet" }}
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
    let (status, set_status) = create_signal(String::from("Ready — connect a wallet and click Pay & Request"));
    let (result, set_result) = create_signal(None::<String>);
    let (tx_hash, set_tx_hash) = create_signal(None::<String>);
    let (loading, set_loading) = create_signal(false);

    let make_request = move |_| {
        let w = wallet.get();
        if !w.connected {
            set_status.set("Connect a wallet first (MetaMask, Demo Key, or Create Wallet)".to_string());
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
