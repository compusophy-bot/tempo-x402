use leptos::prelude::*;
use super::CodeBlock;

#[component]
pub fn EndpointRef() -> impl IntoView {
    view! {
            <div class="card">
                <div class="endpoint-header">
                    <span class="method-badge">"GET"</span>
                    <span class="endpoint-path">"/blockNumber"</span>
                </div>

                <div class="endpoint-meta">
                    <div class="meta-item">
                        <span class="meta-label">"Price"</span>
                        <span class="meta-value" style="color: var(--green)">"$0.001 pathUSD"</span>
                    </div>
                    <div class="meta-item">
                        <span class="meta-label">"Network"</span>
                        <span class="meta-value" style="color: var(--blue)">"Tempo Moderato"</span>
                    </div>
                    <div class="meta-item">
                        <span class="meta-label">"Scheme"</span>
                        <span class="meta-value">"tempo-tip20"</span>
                    </div>
                    <div class="meta-item">
                        <span class="meta-label">"Chain ID"</span>
                        <span class="meta-value">"42431"</span>
                    </div>
                </div>

                <h4 style="font-size: 13px; color: var(--muted); margin-bottom: 12px;">"402 Response (no payment)"</h4>
                <CodeBlock style="margin-bottom: 20px;" html=r#"<span class="hl-cmt">HTTP/1.1 402 Payment Required</span>

{
  <span class="hl-prop">"x402Version"</span>: <span class="hl-num">1</span>,
  <span class="hl-prop">"accepts"</span>: [{
    <span class="hl-prop">"scheme"</span>: <span class="hl-str">"tempo-tip20"</span>,
    <span class="hl-prop">"network"</span>: <span class="hl-str">"eip155:42431"</span>,
    <span class="hl-prop">"price"</span>: <span class="hl-str">"$0.001"</span>,
    <span class="hl-prop">"asset"</span>: <span class="hl-str">"0x20c0...0000"</span>,
    <span class="hl-prop">"payTo"</span>: <span class="hl-str">"0xcdA6...9e25"</span>
  }],
  <span class="hl-prop">"description"</span>: <span class="hl-str">"Get the latest Tempo block number"</span>
}"# />

                <h4 style="font-size: 13px; color: var(--muted); margin-bottom: 12px;">"200 Response (with payment)"</h4>
                <CodeBlock html=r#"<span class="hl-cmt">HTTP/1.1 200 OK</span>

{
  <span class="hl-prop">"blockNumber"</span>: <span class="hl-str">"3427628"</span>
}"# />
            </div>
    }
}
