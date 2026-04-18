#![no_std]
#![no_main]

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! { loop {} }

#[link(wasm_import_module = "x402")]
extern "C" {
    fn response(status: i32, body_ptr: *const u8, body_len: i32, ct_ptr: *const u8, ct_len: i32);
    fn log(level: i32, msg_ptr: *const u8, msg_len: i32);
    fn kv_get(key_ptr: *const u8, key_len: i32) -> i64;
    fn kv_set(key_ptr: *const u8, key_len: i32, val_ptr: *const u8, val_len: i32) -> i32;
    fn payment_info() -> i64;
}

fn respond(status: i32, body: &str, content_type: &str) {
    unsafe {
        response(status, body.as_ptr(), body.len() as i32, content_type.as_ptr(), content_type.len() as i32);
    }
}

fn host_log(level: i32, msg: &str) {
    unsafe { log(level, msg.as_ptr(), msg.len() as i32); }
}

const PAGE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>FAQ</title>
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #fafbfc; color: #1a1a2e; padding: 60px 20px; }
  .container { max-width: 720px; margin: 0 auto; }
  h1 { font-size: 2.2rem; text-align: center; margin-bottom: 8px; }
  .subtitle { text-align: center; color: #6b7280; margin-bottom: 48px; font-size: 1.05rem; }
  .category { font-size: 0.85rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.1em; color: #6366f1; margin: 32px 0 16px; padding-left: 4px; }
  .accordion { border: 1px solid #e5e7eb; border-radius: 12px; overflow: hidden; margin-bottom: 12px; background: #fff; }
  .accordion input[type="checkbox"] { display: none; }
  .accordion label { display: flex; justify-content: space-between; align-items: center; padding: 18px 22px; cursor: pointer; font-weight: 600; font-size: 1rem; color: #1a1a2e; transition: background 0.15s; user-select: none; }
  .accordion label:hover { background: #f9fafb; }
  .accordion label::after { content: '+'; font-size: 1.4rem; color: #9ca3af; transition: transform 0.25s; font-weight: 300; }
  .accordion input:checked + label::after { content: '\2212'; color: #6366f1; }
  .accordion input:checked + label { color: #6366f1; border-bottom: 1px solid #e5e7eb; }
  .answer { max-height: 0; overflow: hidden; transition: max-height 0.35s ease, padding 0.35s ease; }
  .accordion input:checked ~ .answer { max-height: 400px; padding: 18px 22px; }
  .answer p { color: #4b5563; line-height: 1.7; font-size: 0.95rem; }
</style>
</head>
<body>
<div class="container">
  <h1>Frequently Asked Questions</h1>
  <p class="subtitle">Everything you need to know to get started</p>

  <div class="category">Getting Started</div>

  <div class="accordion">
    <input type="checkbox" id="q1">
    <label for="q1">How do I create an account?</label>
    <div class="answer"><p>Click the Sign Up button on our homepage. You can register with your email address or use single sign-on through Google or GitHub. Once confirmed, you will have immediate access to the free tier.</p></div>
  </div>

  <div class="accordion">
    <input type="checkbox" id="q2">
    <label for="q2">Is there a free trial for paid plans?</label>
    <div class="answer"><p>Yes. All paid plans include a 14-day free trial with full access to every feature. No credit card is required to start the trial. You can cancel at any time during the trial period without being charged.</p></div>
  </div>

  <div class="accordion">
    <input type="checkbox" id="q3">
    <label for="q3">What programming languages are supported?</label>
    <div class="answer"><p>Our SDK supports Rust, Python, TypeScript, Go, and Java. Community-maintained libraries also exist for Ruby, C#, and Elixir. Our REST API can be called from any language that supports HTTP requests.</p></div>
  </div>

  <div class="category">Billing &amp; Plans</div>

  <div class="accordion">
    <input type="checkbox" id="q4">
    <label for="q4">Can I upgrade or downgrade at any time?</label>
    <div class="answer"><p>Absolutely. Plan changes take effect immediately. When upgrading, you pay the prorated difference for the remainder of your billing cycle. When downgrading, the unused balance is credited to your next invoice.</p></div>
  </div>

  <div class="accordion">
    <input type="checkbox" id="q5">
    <label for="q5">What payment methods do you accept?</label>
    <div class="answer"><p>We accept all major credit cards (Visa, Mastercard, American Express), PayPal, and wire transfers for annual Enterprise plans. Invoiced billing is available for organizations on the Enterprise tier.</p></div>
  </div>

  <div class="accordion">
    <input type="checkbox" id="q6">
    <label for="q6">What happens if I exceed my API quota?</label>
    <div class="answer"><p>You will receive email and dashboard warnings at 80% and 95% usage. Once you hit 100%, requests will return a 429 status code. You can purchase additional capacity on demand or upgrade your plan for higher limits.</p></div>
  </div>

  <div class="category">Security &amp; Privacy</div>

  <div class="accordion">
    <input type="checkbox" id="q7">
    <label for="q7">Is my data encrypted?</label>
    <div class="answer"><p>All data is encrypted in transit using TLS 1.3 and at rest using AES-256. API keys are stored using one-way hashing. We undergo annual SOC 2 Type II audits and penetration testing by independent firms.</p></div>
  </div>

  <div class="accordion">
    <input type="checkbox" id="q8">
    <label for="q8">Do you sell customer data?</label>
    <div class="answer"><p>Never. We do not sell, rent, or share your data with third parties for marketing purposes. Your data is yours. Please review our Privacy Policy for complete details on how we handle information.</p></div>
  </div>

  <div class="accordion">
    <input type="checkbox" id="q9">
    <label for="q9">Where are your servers located?</label>
    <div class="answer"><p>Our infrastructure runs across three regions: US East (Virginia), EU West (Frankfurt), and Asia Pacific (Tokyo). Enterprise customers can choose their preferred data residency region to meet compliance requirements.</p></div>
  </div>
</div>
</body>
</html>"#;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    host_log(1, "serving faq page");
    respond(200, PAGE, "text/html; charset=utf-8");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
