use leptos::prelude::*;

mod components;
use components::*;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Tab {
    Demo,
    Protocol,
    Integrate,
    Agent,
}

fn main() {
    console_log::init_with_level(log::Level::Debug).expect("console_log init");
    leptos::mount::mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    let (active_tab, set_active_tab) = signal(Tab::Demo);

    let tab_btn = move |tab: Tab, label: &'static str| {
        let is_active = move || active_tab.get() == tab;
        view! {
            <button
                class:tab=true
                class:active=is_active
                on:click=move |_| set_active_tab.set(tab)
            >
                {label}
            </button>
        }
    };

    view! {
        <div class="app-shell">
            <Hero />
            <nav class="tab-bar">
                {tab_btn(Tab::Demo, "Demo")}
                {tab_btn(Tab::Protocol, "Protocol")}
                {tab_btn(Tab::Integrate, "Integrate")}
                {tab_btn(Tab::Agent, "Agent")}
            </nav>
            <div class="tab-content">
                <div class="tab-panel" style:display=move || if active_tab.get() == Tab::Demo { "block" } else { "none" }>
                    <LiveDemo />
                </div>
                <div class="tab-panel" style:display=move || if active_tab.get() == Tab::Protocol { "block" } else { "none" }>
                    <HowItWorks />
                    <div style="margin-top: 24px;">
                        <h3 style="font-size: 16px; font-weight: 600; margin-bottom: 12px;">"Endpoint Reference"</h3>
                        <EndpointRef />
                    </div>
                </div>
                <div class="tab-panel" style:display=move || if active_tab.get() == Tab::Integrate { "block" } else { "none" }>
                    <CodeExamples />
                </div>
                <div class="tab-panel" style:display=move || if active_tab.get() == Tab::Agent { "block" } else { "none" }>
                    <AgentOnboarding />
                </div>
            </div>
            <Footer />
        </div>
    }
}
