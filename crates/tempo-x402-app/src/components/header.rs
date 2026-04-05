use leptos::*;

/// Minimal header — only used by Studio page.
#[component]
pub fn Header() -> impl IntoView {
    view! { <div></div> }
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
