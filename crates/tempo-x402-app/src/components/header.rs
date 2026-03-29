use leptos::*;

use super::wallet_panel::WalletButtons;
use crate::*;

/// Header with navigation and wallet connection
#[component]
pub fn Header() -> impl IntoView {
    let (wallet, set_wallet) =
        expect_context::<(ReadSignal<WalletState>, WriteSignal<WalletState>)>();

    let location = use_location();
    let (mobile_open, set_mobile_open) = create_signal(false);

    let toggle_mobile = move |_| set_mobile_open.update(|v| *v = !*v);

    view! {
        <header class="header">
            <nav class="nav">
                <a href="/" class="logo">"tempo-x402"</a>
                <button class="mobile-nav-toggle" on:click=toggle_mobile>
                    {move || if mobile_open.get() { "\u{2715}" } else { "\u{2630}" }}
                </button>
                <div class=move || {
                    if mobile_open.get() { "nav-links open" } else { "nav-links" }
                }>
                    {move || {
                        let path = location.pathname.get();
                        view! {
                            <a
                                href="/dashboard"
                                class=if path == "/dashboard" { "active" } else { "" }
                                on:click=move |_| set_mobile_open.set(false)
                            >"Dashboard"</a>
                            <a
                                href="/studio"
                                class=if path == "/studio" { "active" } else { "" }
                                on:click=move |_| set_mobile_open.set(false)
                            >"Studio"</a>
                            <a
                                href="/cartridges"
                                class=if path == "/cartridges" { "active" } else { "" }
                                on:click=move |_| set_mobile_open.set(false)
                            >"Cartridges"</a>
                            <a
                                href="/timeline"
                                class=if path == "/timeline" { "active" } else { "" }
                                on:click=move |_| set_mobile_open.set(false)
                            >"Timeline"</a>
                        }
                    }}
                </div>
                <WalletButtons wallet=wallet set_wallet=set_wallet />
            </nav>
        </header>
    }
}

/// Footer with version and external links
#[component]
pub fn Footer() -> impl IntoView {
    view! {
        <footer class="footer">
            <p>
                <a href="https://docs.rs/tempo-x402" target="_blank">"docs"</a>
                " \u{00B7} "
                <a href="https://crates.io/crates/tempo-x402" target="_blank">"crates"</a>
                " \u{00B7} "
                <a href="https://github.com/compusophy/tempo-x402" target="_blank">"github"</a>
            </p>
            <p class="footer-version">{concat!("tempo-x402 v", env!("CARGO_PKG_VERSION"))}</p>
        </footer>
    }
}

/// 404 page
#[component]
pub fn NotFound() -> impl IntoView {
    view! {
        <div class="page">
            <h1>"404 - Not Found"</h1>
            <p><a href="/">"Go home"</a></p>
        </div>
    }
}
