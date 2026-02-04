use leptos::prelude::*;
use super::CodeBlock;

#[component]
pub fn CodeExamples() -> impl IntoView {
    view! {
            <div class="examples-grid">
                <div class="example-card">
                    <h3>"TypeScript (x402/fetch)"</h3>
                    <CodeBlock html=r##"<span class="hl-kw">import</span> { x402Client, wrapFetchWithPayment } <span class="hl-kw">from</span> <span class="hl-str">"@x402/fetch"</span>;
<span class="hl-kw">import</span> { privateKeyToAccount } <span class="hl-kw">from</span> <span class="hl-str">"viem/accounts"</span>;
<span class="hl-kw">import</span> { registerTempoScheme } <span class="hl-kw">from</span> <span class="hl-str">"./tempo-scheme.js"</span>;

<span class="hl-kw">const</span> signer = <span class="hl-fn">privateKeyToAccount</span>(<span class="hl-str">"0x..."</span>);
<span class="hl-kw">const</span> client = <span class="hl-kw">new</span> <span class="hl-fn">x402Client</span>();
<span class="hl-fn">registerTempoScheme</span>(client, signer);

<span class="hl-kw">const</span> fetchPaid = <span class="hl-fn">wrapFetchWithPayment</span>(fetch, client);
<span class="hl-kw">const</span> res = <span class="hl-kw">await</span> <span class="hl-fn">fetchPaid</span>(<span class="hl-str">"http://localhost:4021/blockNumber"</span>);
<span class="hl-kw">const</span> data = <span class="hl-kw">await</span> res.<span class="hl-fn">json</span>();
console.<span class="hl-fn">log</span>(data); <span class="hl-cmt">// { blockNumber: "3427628" }</span>"## />
                </div>

                <div class="example-card">
                    <h3>"Rust (x402-client)"</h3>
                    <CodeBlock html=r##"<span class="hl-kw">use</span> x402_tempo::<span class="hl-type">TempoSchemeClient</span>;
<span class="hl-kw">use</span> x402_client::<span class="hl-type">X402Client</span>;
<span class="hl-kw">use</span> alloy::signers::local::<span class="hl-type">PrivateKeySigner</span>;

<span class="hl-kw">let</span> signer: <span class="hl-type">PrivateKeySigner</span> = key.<span class="hl-fn">parse</span>()?;
<span class="hl-kw">let</span> scheme = <span class="hl-type">TempoSchemeClient</span>::<span class="hl-fn">new</span>(signer);
<span class="hl-kw">let</span> client = <span class="hl-type">X402Client</span>::<span class="hl-fn">new</span>(scheme);

<span class="hl-kw">let</span> (resp, settle) = client
    .<span class="hl-fn">fetch</span>(<span class="hl-str">"http://localhost:4021/blockNumber"</span>, <span class="hl-type">Method</span>::GET)
    .<span class="hl-kw">await</span>?;
<span class="hl-cmt">// blockNumber: "3427628"</span>"## />
                </div>

                <div class="example-card">
                    <h3>"curl (manual flow)"</h3>
                    <CodeBlock html=r##"<span class="hl-cmt"># Step 1 — get payment requirements</span>
<span class="hl-fn">curl</span> <span class="hl-str">http://localhost:4021/blockNumber</span>
<span class="hl-cmt"># → 402 { "accepts": [{ "scheme": "tempo-tip20", ... }] }</span>

<span class="hl-cmt"># Step 2 — sign & attach payment (handled by x402 client)</span>
<span class="hl-fn">curl</span> <span class="hl-op">-H</span> <span class="hl-str">"X-PAYMENT: &lt;signed-payload&gt;"</span> \
     <span class="hl-str">http://localhost:4021/blockNumber</span>
<span class="hl-cmt"># → 200 { "blockNumber": "3427628" }</span>"## />
                </div>
            </div>
    }
}
