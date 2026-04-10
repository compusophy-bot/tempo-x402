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
<title>Privacy Policy</title>
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #fff; color: #374151; line-height: 1.75; }
  .banner { background: linear-gradient(to right, #1e3a5f, #2563eb); padding: 52px 20px; text-align: center; }
  .banner h1 { color: #fff; font-size: 2.2rem; margin-bottom: 6px; }
  .banner p { color: rgba(255,255,255,0.7); font-size: 0.95rem; }
  .content { max-width: 740px; margin: 0 auto; padding: 48px 24px 80px; }
  h2 { font-size: 1.25rem; color: #111827; margin: 32px 0 10px; }
  p, li { font-size: 0.98rem; margin-bottom: 12px; }
  ul { margin: 0 0 16px 24px; }
  ul li { margin-bottom: 6px; }
  table { width: 100%; border-collapse: collapse; margin: 16px 0 24px; }
  th, td { text-align: left; padding: 12px 14px; border-bottom: 1px solid #e5e7eb; font-size: 0.9rem; }
  th { background: #f9fafb; color: #111827; font-weight: 700; }
  td { color: #4b5563; }
  .info-box { background: #eff6ff; border: 1px solid #bfdbfe; border-radius: 8px; padding: 16px 20px; margin: 20px 0; font-size: 0.93rem; color: #1e40af; }
  .contact { background: #f9fafb; border-radius: 12px; padding: 24px; margin-top: 36px; }
  .contact h3 { font-size: 1.1rem; margin-bottom: 8px; }
</style>
</head>
<body>
<div class="banner">
  <h1>Privacy Policy</h1>
  <p>Effective date: April 1, 2026</p>
</div>
<div class="content">
  <p>We respect your privacy and are committed to protecting the personal data you share with us. This policy explains what data we collect, how we use it, and your rights regarding that data.</p>

  <h2>Information We Collect</h2>
  <table>
    <tr><th>Category</th><th>Examples</th><th>Purpose</th></tr>
    <tr><td>Account Data</td><td>Name, email, password hash</td><td>Authentication, communication</td></tr>
    <tr><td>Usage Data</td><td>API calls, feature usage, timestamps</td><td>Service improvement, billing</td></tr>
    <tr><td>Device Data</td><td>Browser type, OS, IP address</td><td>Security, fraud prevention</td></tr>
    <tr><td>Payment Data</td><td>Card last-4, billing address</td><td>Payment processing</td></tr>
  </table>

  <h2>How We Use Your Data</h2>
  <ul>
    <li>To provide, maintain, and improve our services</li>
    <li>To process transactions and send billing notifications</li>
    <li>To detect and prevent security threats and fraud</li>
    <li>To communicate service updates and respond to support requests</li>
    <li>To comply with legal obligations</li>
  </ul>

  <h2>Data Sharing</h2>
  <p>We do not sell your personal data. We may share limited data with:</p>
  <ul>
    <li><strong>Service providers</strong> who assist in operating our platform (hosting, payment processing, analytics)</li>
    <li><strong>Legal authorities</strong> when required by law or to protect our rights</li>
    <li><strong>Business transfers</strong> in connection with a merger, acquisition, or asset sale</li>
  </ul>
  <div class="info-box">All third-party processors are contractually bound to handle your data in accordance with this policy and applicable data protection laws.</div>

  <h2>Data Retention</h2>
  <p>We retain account data for the duration of your account plus 30 days. Usage logs are retained for 90 days. Payment records are retained for 7 years as required by financial regulations. You may request deletion of your data at any time.</p>

  <h2>Your Rights</h2>
  <p>Depending on your jurisdiction, you may have the right to:</p>
  <ul>
    <li>Access the personal data we hold about you</li>
    <li>Correct inaccurate or incomplete data</li>
    <li>Request deletion of your data</li>
    <li>Object to or restrict certain processing activities</li>
    <li>Export your data in a portable format</li>
    <li>Withdraw consent at any time</li>
  </ul>

  <h2>Cookies</h2>
  <p>We use strictly necessary cookies for session management and authentication. We use analytics cookies only with your explicit consent. You can manage cookie preferences through your browser settings or our cookie banner.</p>

  <h2>Security</h2>
  <p>We implement industry-standard security measures including TLS 1.3 encryption, AES-256 encryption at rest, regular penetration testing, and SOC 2 Type II compliance. Despite these measures, no system is completely secure, and we cannot guarantee absolute security.</p>

  <div class="contact">
    <h3>Contact Us</h3>
    <p>For privacy inquiries or to exercise your rights, contact our Data Protection Officer at privacy@example.com or write to: Privacy Team, 123 Innovation Drive, San Francisco, CA 94105.</p>
  </div>
</div>
</body>
</html>"#;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    host_log(1, "serving privacy policy");
    respond(200, PAGE, "text/html; charset=utf-8");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
