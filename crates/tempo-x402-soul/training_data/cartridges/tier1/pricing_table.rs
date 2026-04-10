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
<title>Pricing</title>
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #f0f2f5; color: #1a1a2e; padding: 60px 20px; }
  h1 { text-align: center; font-size: 2.4rem; margin-bottom: 12px; }
  .subtitle { text-align: center; color: #6b7280; font-size: 1.1rem; margin-bottom: 48px; }
  .grid { display: flex; justify-content: center; gap: 28px; flex-wrap: wrap; max-width: 1100px; margin: 0 auto; }
  .card { background: #fff; border-radius: 16px; padding: 40px 32px; width: 320px; box-shadow: 0 4px 24px rgba(0,0,0,0.08); transition: transform 0.2s, box-shadow 0.2s; position: relative; }
  .card:hover { transform: translateY(-4px); box-shadow: 0 8px 32px rgba(0,0,0,0.14); }
  .card.featured { border: 2px solid #6366f1; }
  .card.featured::before { content: 'MOST POPULAR'; position: absolute; top: -14px; left: 50%; transform: translateX(-50%); background: #6366f1; color: #fff; font-size: 0.75rem; font-weight: 700; padding: 4px 16px; border-radius: 20px; letter-spacing: 0.05em; }
  .tier { font-size: 1rem; font-weight: 600; text-transform: uppercase; letter-spacing: 0.08em; color: #6366f1; margin-bottom: 8px; }
  .price { font-size: 3rem; font-weight: 800; margin-bottom: 4px; }
  .price span { font-size: 1rem; font-weight: 400; color: #9ca3af; }
  .desc { color: #6b7280; font-size: 0.95rem; margin-bottom: 28px; line-height: 1.5; }
  ul { list-style: none; margin-bottom: 32px; }
  ul li { padding: 10px 0; border-bottom: 1px solid #f3f4f6; font-size: 0.95rem; display: flex; align-items: center; gap: 10px; }
  ul li::before { content: '\2713'; color: #10b981; font-weight: 700; font-size: 1.1rem; }
  .btn { display: block; width: 100%; padding: 14px; border: none; border-radius: 10px; font-size: 1rem; font-weight: 600; cursor: pointer; text-align: center; transition: background 0.2s; }
  .btn-outline { background: transparent; border: 2px solid #6366f1; color: #6366f1; }
  .btn-outline:hover { background: #eef2ff; }
  .btn-primary { background: #6366f1; color: #fff; }
  .btn-primary:hover { background: #4f46e5; }
  .btn-dark { background: #1a1a2e; color: #fff; }
  .btn-dark:hover { background: #16162a; }
</style>
</head>
<body>
  <h1>Simple, transparent pricing</h1>
  <p class="subtitle">No hidden fees. Upgrade or downgrade at any time.</p>
  <div class="grid">
    <div class="card">
      <div class="tier">Free</div>
      <div class="price">$0<span>/mo</span></div>
      <p class="desc">Perfect for side projects and trying things out.</p>
      <ul>
        <li>1,000 API requests/mo</li>
        <li>1 team member</li>
        <li>Community support</li>
        <li>Basic analytics</li>
        <li>48h data retention</li>
      </ul>
      <button class="btn btn-outline">Get Started Free</button>
    </div>
    <div class="card featured">
      <div class="tier">Pro</div>
      <div class="price">$29<span>/mo</span></div>
      <p class="desc">For growing teams that need more power and flexibility.</p>
      <ul>
        <li>100,000 API requests/mo</li>
        <li>Up to 10 team members</li>
        <li>Priority email support</li>
        <li>Advanced analytics</li>
        <li>30-day data retention</li>
        <li>Custom webhooks</li>
        <li>SSO integration</li>
      </ul>
      <button class="btn btn-primary">Start Pro Trial</button>
    </div>
    <div class="card">
      <div class="tier">Enterprise</div>
      <div class="price">$149<span>/mo</span></div>
      <p class="desc">For organizations requiring scale, security, and dedicated support.</p>
      <ul>
        <li>Unlimited API requests</li>
        <li>Unlimited team members</li>
        <li>24/7 dedicated support</li>
        <li>Real-time analytics</li>
        <li>1-year data retention</li>
        <li>Custom webhooks</li>
        <li>SSO &amp; SAML</li>
        <li>SLA guarantee 99.99%</li>
        <li>Dedicated account manager</li>
      </ul>
      <button class="btn btn-dark">Contact Sales</button>
    </div>
  </div>
</body>
</html>"#;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    host_log(1, "serving pricing table");
    respond(200, PAGE, "text/html; charset=utf-8");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
