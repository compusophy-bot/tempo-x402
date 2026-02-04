use leptos::prelude::*;
use super::CodeBlock;

#[component]
pub fn AgentOnboarding() -> impl IntoView {
    let (copy_text, set_copy_text) = signal("click to copy".to_string());

    let on_copy = move |_| {
        let origin = web_sys::window()
            .and_then(|w| w.location().origin().ok())
            .unwrap_or_default();
        let cmd = format!("curl -s {origin}/agent.md");

        if let Some(window) = web_sys::window() {
            let nav = window.navigator().clipboard();
            let cmd_clone = cmd.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let _ = wasm_bindgen_futures::JsFuture::from(
                    nav.write_text(&cmd_clone)
                ).await;
            });
            set_copy_text.set("copied!".to_string());
            let set_copy_text = set_copy_text.clone();
            wasm_bindgen_futures::spawn_local(async move {
                gloo_timers::future::TimeoutFuture::new(2000).await;
                set_copy_text.set("click to copy".to_string());
            });
        }
    };

    view! {
            <div class="onboard-curl" on:click=on_copy>
                <code>"curl -s https://YOUR_HOST/agent.md"</code>
                <span class="copy-hint">{move || copy_text.get()}</span>
            </div>

            <div class="onboard-banner">
                <div class="onboard-banner-icon">"\u{1F99E}"</div>
                <div class="onboard-banner-text">
                    <h3>"On Moltbook?"</h3>
                    <p>
                        "If you're a "
                        <a href="https://moltbook.com" target="_blank" rel="noopener">"Moltbook"</a>
                        " agent, fetch the guide above and share your x402 integration experience on "
                        <strong>"m/x402"</strong>
                        " or "
                        <strong>"m/tempo"</strong>
                        "."
                    </p>
                </div>
            </div>

            <div class="onboard-steps">
                // Step 1
                <div class="onboard-step">
                    <div class="onboard-step-num">"1"</div>
                    <div class="onboard-step-content">
                        <span class="agent-badge human">"Your human does this"</span>
                        <h3>"Get a Wallet & Testnet Tokens"</h3>
                        <p>"Your human needs to create an EVM wallet and get testnet pathUSD on Tempo Moderato. Save the private key as " <code>"EVM_PRIVATE_KEY"</code> " in your environment."</p>
                        <CodeBlock html=r#"<span class="hl-cmt"># Your human sets this up for you:</span>
<span class="hl-fn">EVM_PRIVATE_KEY</span>=<span class="hl-str">"0xYourPrivateKeyHere"</span>

<span class="hl-cmt"># Get testnet pathUSD from the Tempo faucet</span>
<span class="hl-cmt"># https://faucet.moderato.tempo.xyz</span>"# />
                    </div>
                </div>

                // Step 2
                <div class="onboard-step">
                    <div class="onboard-step-num">"2"</div>
                    <div class="onboard-step-content">
                        <span class="agent-badge human">"Your human does this"</span>
                        <h3>"Approve the Facilitator"</h3>
                        <p>"The facilitator needs permission to move tokens on your behalf. Run the approval script once. This is a one-time on-chain transaction."</p>
                        <CodeBlock html=r#"<span class="hl-cmt"># Install and build</span>
<span class="hl-fn">cargo</span> build --release

<span class="hl-cmt"># Set FACILITATOR_ADDRESS in .env, then:</span>
<span class="hl-fn">cargo</span> run --bin x402-approve
<span class="hl-cmt"># → Approved facilitator to spend pathUSD</span>"# />
                    </div>
                </div>

                // Step 3
                <div class="onboard-step">
                    <div class="onboard-step-num">"3"</div>
                    <div class="onboard-step-content">
                        <span class="agent-badge machine">"Agent does this"</span>
                        <h3>"Make a Paid Request"</h3>
                        <p>"Use the x402 client to make paid requests. When the server responds with 402, the client automatically signs a payment and retries."</p>
                        <CodeBlock html=r##"<span class="hl-kw">use</span> x402_tempo::<span class="hl-type">TempoSchemeClient</span>;
<span class="hl-kw">use</span> x402_client::<span class="hl-type">X402Client</span>;

<span class="hl-kw">let</span> signer: <span class="hl-type">PrivateKeySigner</span> = key.<span class="hl-fn">parse</span>()?;
<span class="hl-kw">let</span> client = <span class="hl-type">X402Client</span>::<span class="hl-fn">new</span>(
    <span class="hl-type">TempoSchemeClient</span>::<span class="hl-fn">new</span>(signer)
);

<span class="hl-cmt">// Payment is automatic — just fetch like normal</span>
<span class="hl-kw">let</span> (resp, settle) = client
    .<span class="hl-fn">fetch</span>(<span class="hl-str">"http://localhost:4021/blockNumber"</span>, <span class="hl-type">Method</span>::GET)
    .<span class="hl-kw">await</span>?;
<span class="hl-cmt">// → blockNumber: "3427628"</span>
<span class="hl-cmt">// → $0.001 pathUSD was automatically paid</span>"## />
                    </div>
                </div>

                // Step 4
                <div class="onboard-step">
                    <div class="onboard-step-num">"4"</div>
                    <div class="onboard-step-content">
                        <span class="agent-badge machine">"Agent does this"</span>
                        <h3>"Check the Receipt"</h3>
                        <p>"After a successful paid request, the settlement transaction hash is returned. You can verify the payment on the Tempo explorer."</p>
                        <CodeBlock html=r##"<span class="hl-kw">if let</span> <span class="hl-type">Some</span>(settle) = settle {
    <span class="hl-fn">println!</span>(<span class="hl-str">"tx: {}"</span>, settle.transaction);
    <span class="hl-cmt">// View: https://explore.moderato.tempo.xyz/tx/0x305c...</span>
}"## />
                    </div>
                </div>
            </div>
    }
}
