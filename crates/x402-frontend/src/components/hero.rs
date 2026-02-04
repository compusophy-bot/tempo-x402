use leptos::prelude::*;

#[component]
pub fn Hero() -> impl IntoView {
    view! {
        <header class="hero-compact">
            <div class="hero-left">
                <span class="hero-title">
                    <span class="accent-green">"x402"</span>
                    " on "
                    <span class="accent-blue">"Tempo"</span>
                </span>
                <span class="hero-tagline">"Pay-per-request API monetization"</span>
            </div>
            <div class="hero-right">
                <a href="#" class="pill-link">"Docs"</a>
                <a href="#" class="pill-link">"GitHub"</a>
            </div>
        </header>
    }
}
