use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

mod api;
pub mod cartridge_runner;
mod components;
pub mod studio;
mod wallet;
mod wallet_crypto;

use components::cockpit::CockpitPage;
use components::mandala::Mandala;
use components::nav::NavBar;

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
    pub fn label(&self) -> &'static str {
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
        <Meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover" />
        <Meta name="apple-mobile-web-app-capable" content="yes" />
        <Meta name="apple-mobile-web-app-status-bar-style" content="black-translucent" />
        <Meta name="theme-color" content="#040408" />
        <Title text="tempo-x402" />
        <Stylesheet href="/style.css" />

        <Router>
            <div class="app-shell">
                <NavBar />
                <main class="app-content">
                    <Routes>
                        <Route path="/" view=Mandala />
                        <Route path="/dashboard" view=CockpitPage />
                        <Route path="/studio" view=studio::StudioPage />
                        <Route path="/*any" view=Mandala />
                    </Routes>
                </main>
            </div>
        </Router>
    }
}

/// Initialize the app
#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(|| view! { <App /> });
}
