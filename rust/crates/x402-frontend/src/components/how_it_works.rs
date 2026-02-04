use leptos::prelude::*;

#[component]
pub fn HowItWorks() -> impl IntoView {
    let steps = vec![
        ("1", "blue", "Client Request", "Client sends ", "GET /blockNumber", " without any payment header."),
        ("2", "orange", "402 Payment Required", "Server responds with ", "402", " and a JSON body describing what payment is accepted: scheme, network, price, and recipient address."),
        ("3", "blue", "Sign Payment", "Client signs an EIP-712 ", "PaymentAuthorization", " message with its wallet. No tokens move yet \u{2014} this is just authorization."),
        ("4", "green", "Facilitator Verifies", "The facilitator checks the signature, validates the time window, and confirms on-chain balance and allowance.", "", ""),
        ("5", "green", "On-Chain Settlement", "The facilitator calls ", "transferFrom", " on the TIP-20 contract to move tokens from payer to payee. A real transaction is mined on Tempo."),
        ("6", "green", "Response Delivered", "Server returns the protected resource along with settlement proof (transaction hash) in the response headers.", "", ""),
    ];

    view! {
            <div class="flow-steps">
                {steps.into_iter().map(|(num, color, title, pre_code, code_text, post_code)| {
                    view! {
                        <div class="flow-step">
                            <div class={format!("step-num {color}")}>{num}</div>
                            <div class="step-content">
                                <h3>{title}</h3>
                                <p>
                                    {pre_code}
                                    {if !code_text.is_empty() {
                                        Some(view! { <code>{code_text}</code> })
                                    } else {
                                        None
                                    }}
                                    {post_code}
                                </p>
                            </div>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>
    }
}
