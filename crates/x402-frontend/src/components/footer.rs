use leptos::prelude::*;

#[component]
pub fn Footer() -> impl IntoView {
    view! {
        <footer class="footer-compact">
            "Built with "
            <a href="https://x402.org" target="_blank" rel="noopener">"x402"</a>
            " \u{00B7} "
            <a href="https://tempo.xyz" target="_blank" rel="noopener">"Tempo"</a>
            " Moderato"
        </footer>
    }
}
