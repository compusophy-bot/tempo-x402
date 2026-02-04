use leptos::prelude::*;

#[component]
pub fn CodeBlock(
    html: &'static str,
    #[prop(default = "")]
    style: &'static str,
) -> impl IntoView {
    let node_ref = NodeRef::new();

    Effect::new(move |_| {
        let el: Option<web_sys::HtmlDivElement> = node_ref.get();
        if let Some(el) = el {
            el.set_inner_html(html);
        }
    });

    view! {
        <div class="code-block" style=style node_ref=node_ref></div>
    }
}
