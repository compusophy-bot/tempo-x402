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
                    <Route path="/gateway" view=GatewayPage />
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
                    <a href="/gateway">"Gateway"</a>
                    <a href="/docs">"Docs"</a>
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
        match wallet::create_embedded_wallet() {
            Ok(state) => {
                let address = state.address.clone().unwrap_or_default();
                set_wallet.set(state);
                // Fund the new wallet
                spawn_local(async move {
                    match wallet::fund_address(&address).await {
                        Ok(_) => {
                            web_sys::console::log_1(&format!("Funded wallet: {}", address).into());
                        }
                        Err(e) => {
                            web_sys::console::error_1(&format!("Funding failed: {}", e).into());
                        }
                    }
                    set_funding.set(false);
                });
            }
            Err(e) => {
                web_sys::console::error_1(&format!("Create wallet error: {}", e).into());
                set_funding.set(false);
            }
        }
    };

    let disconnect = move |_| {
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
                        {move || if funding.get() { "Creating..." } else { "Create Wallet" }}
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
                "HTTP 402 Payment Required - Pay-per-request APIs on Tempo blockchain"
            </p>

            <PaymentDemo />

            <div class="info-section">
                <h2>"How it works"</h2>
                <ol class="steps">
                    <li>"Connect your wallet with pathUSD tokens"</li>
                    <li>"Request a protected endpoint"</li>
                    <li>"Receive 402 with payment requirements"</li>
                    <li>"Sign an EIP-712 payment authorization"</li>
                    <li>"Retry with PAYMENT-SIGNATURE header"</li>
                    <li>"Facilitator verifies and settles on-chain"</li>
                    <li>"Receive content + transaction hash"</li>
                </ol>
            </div>
        </div>
    }
}

/// Payment demo component
#[component]
fn PaymentDemo() -> impl IntoView {
    let (wallet, _) = expect_context::<(ReadSignal<WalletState>, WriteSignal<WalletState>)>();
    let (status, set_status) = create_signal(String::from("Ready to make a paid request"));
    let (result, set_result) = create_signal(None::<String>);
    let (tx_hash, set_tx_hash) = create_signal(None::<String>);
    let (loading, set_loading) = create_signal(false);

    let make_request = move |_| {
        let w = wallet.get();
        if !w.connected {
            set_status.set("Please connect your wallet first".to_string());
            return;
        }

        set_loading.set(true);
        set_status.set("Requesting protected endpoint...".to_string());
        set_result.set(None);
        set_tx_hash.set(None);

        spawn_local(async move {
            match api::make_paid_request(&w).await {
                Ok((data, settle)) => {
                    if let Some(ref s) = settle {
                        if s.success {
                            set_status.set("Payment successful!".to_string());
                        } else {
                            set_status.set("Payment settled (check tx)".to_string());
                        }
                        set_tx_hash.set(s.transaction.clone());
                    } else {
                        set_status.set("Response received (no payment required)".to_string());
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

            <div class="demo-controls">
                <button
                    class="btn btn-primary"
                    on:click=make_request
                    disabled=move || loading.get()
                >
                    {move || if loading.get() { "Processing..." } else { "Pay & Request" }}
                </button>
            </div>

            <div class="demo-status">
                <p class="status-text">{move || status.get()}</p>
            </div>

            <Show when=move || result.get().is_some() fallback=|| ()>
                <div class="demo-result">
                    <h4>"Response"</h4>
                    <pre class="code-block">{move || result.get().unwrap_or_default()}</pre>
                </div>
            </Show>

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
        </div>
    }
}

/// Gateway management page
#[component]
fn GatewayPage() -> impl IntoView {
    view! {
        <div class="page">
            <h1>"API Gateway"</h1>
            <p class="subtitle">
                "Register your API endpoints and add payment rails"
            </p>

            <EndpointRegistration />
            <EndpointList />
        </div>
    }
}

/// Endpoint registration form
#[component]
fn EndpointRegistration() -> impl IntoView {
    let (slug, set_slug) = create_signal(String::new());
    let (target_url, set_target_url) = create_signal(String::new());
    let (price, set_price) = create_signal(String::from("$0.01"));
    let (status, set_status) = create_signal(String::new());
    let (loading, set_loading) = create_signal(false);

    let register = move |_| {
        set_loading.set(true);
        set_status.set("Registering endpoint...".to_string());

        let s = slug.get();
        let t = target_url.get();
        let p = price.get();

        spawn_local(async move {
            match api::register_endpoint(&s, &t, &p).await {
                Ok(_) => {
                    set_status.set("Endpoint registered successfully!".to_string());
                    set_slug.set(String::new());
                    set_target_url.set(String::new());
                }
                Err(e) => {
                    set_status.set(format!("Error: {}", e));
                }
            }
            set_loading.set(false);
        });
    };

    view! {
        <div class="card">
            <h3>"Register New Endpoint"</h3>

            <div class="form-group">
                <label>"Slug"</label>
                <input
                    type="text"
                    placeholder="my-api"
                    prop:value=move || slug.get()
                    on:input=move |ev| set_slug.set(event_target_value(&ev))
                />
            </div>

            <div class="form-group">
                <label>"Target URL"</label>
                <input
                    type="url"
                    placeholder="https://api.example.com"
                    prop:value=move || target_url.get()
                    on:input=move |ev| set_target_url.set(event_target_value(&ev))
                />
            </div>

            <div class="form-group">
                <label>"Price per request"</label>
                <input
                    type="text"
                    placeholder="$0.01"
                    prop:value=move || price.get()
                    on:input=move |ev| set_price.set(event_target_value(&ev))
                />
            </div>

            <button
                class="btn btn-primary"
                on:click=register
                disabled=move || loading.get()
            >
                {move || if loading.get() { "Registering..." } else { "Register (0.01 pathUSD)" }}
            </button>

            <Show when=move || !status.get().is_empty() fallback=|| ()>
                <p class="status-text">{move || status.get()}</p>
            </Show>
        </div>
    }
}

/// List of registered endpoints
#[component]
fn EndpointList() -> impl IntoView {
    let (endpoints, set_endpoints) = create_signal(Vec::<serde_json::Value>::new());
    let (loading, set_loading) = create_signal(true);

    // Load endpoints on mount
    create_effect(move |_| {
        spawn_local(async move {
            match api::list_endpoints().await {
                Ok(list) => set_endpoints.set(list),
                Err(e) => {
                    web_sys::console::error_1(&format!("Error: {}", e).into());
                }
            }
            set_loading.set(false);
        });
    });

    view! {
        <div class="card">
            <h3>"Registered Endpoints"</h3>

            <Show
                when=move || !loading.get()
                fallback=|| view! { <p>"Loading..."</p> }
            >
                <Show
                    when=move || !endpoints.get().is_empty()
                    fallback=|| view! { <p class="empty">"No endpoints registered yet"</p> }
                >
                    <ul class="endpoint-list">
                        <For
                            each=move || endpoints.get()
                            key=|ep| ep["slug"].as_str().unwrap_or("").to_string()
                            children=move |ep| {
                                let slug = ep["slug"].as_str().unwrap_or("").to_string();
                                let price = ep["price_usd"].as_str().unwrap_or("").to_string();
                                let target = ep["target_url"].as_str().unwrap_or("").to_string();
                                view! {
                                    <li class="endpoint-item">
                                        <div class="endpoint-slug">{slug}</div>
                                        <div class="endpoint-price">{price}</div>
                                        <div class="endpoint-target">{target}</div>
                                    </li>
                                }
                            }
                        />
                    </ul>
                </Show>
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
                        <li><a href="https://crates.io/crates/tempo-x402">"tempo-x402"</a>" - Core types and crypto"</li>
                        <li><a href="https://crates.io/crates/tempo-x402-client">"tempo-x402-client"</a>" - Client SDK"</li>
                        <li><a href="https://crates.io/crates/tempo-x402-server">"tempo-x402-server"</a>" - Server middleware"</li>
                        <li><a href="https://crates.io/crates/tempo-x402-facilitator">"tempo-x402-facilitator"</a>" - Payment settlement"</li>
                        <li><a href="https://crates.io/crates/tempo-x402-gateway">"tempo-x402-gateway"</a>" - API gateway"</li>
                        <li><a href="https://crates.io/crates/tempo-x402-wallet">"tempo-x402-wallet"</a>" - WASM wallet (signing + key gen)"</li>
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
