use leptos::*;
use leptos_router::*;

/// Shared navigation bar — appears on all pages.
#[component]
pub fn NavBar() -> impl IntoView {
    let location = use_location();
    let pathname = move || location.pathname.get();

    let link = |href: &'static str, label: &'static str| {
        let class = move || {
            let p = pathname();
            let active = if href == "/" {
                p == "/"
            } else {
                p.starts_with(href)
            };
            if active {
                "app-nav-link active"
            } else {
                "app-nav-link"
            }
        };
        view! {
            <A href=href class=class>{label}</A>
        }
    };

    view! {
        <nav class="app-nav">
            <span class="app-nav-brand">"x402"</span>
            {link("/", "Colony")}
            {link("/dashboard", "Cockpit")}
            {link("/studio", "Studio")}
        </nav>
    }
}
