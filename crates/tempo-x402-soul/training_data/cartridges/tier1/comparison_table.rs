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
    unsafe { response(status, body.as_ptr(), body.len() as i32, content_type.as_ptr(), content_type.len() as i32); }
}

fn host_log(level: i32, msg: &str) {
    unsafe { log(level, msg.as_ptr(), msg.len() as i32); }
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}

const BODY: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Feature Comparison</title>
<style>
  *, *::before, *::after { margin: 0; padding: 0; box-sizing: border-box; }
  body { font-family: 'Segoe UI', system-ui, sans-serif; background: #f8fafc; color: #1e293b; padding: 60px 20px; }
  .container { max-width: 900px; margin: 0 auto; }
  .header { text-align: center; margin-bottom: 48px; }
  .header h1 { font-size: 2rem; font-weight: 800; margin-bottom: 8px; }
  .header p { color: #64748b; font-size: 1.05rem; }
  .table-wrap { overflow-x: auto; border-radius: 16px; box-shadow: 0 4px 24px rgba(0,0,0,0.06); }
  table { width: 100%; border-collapse: collapse; background: #fff; }
  thead th {
    padding: 24px 20px; text-align: center; font-size: 0.95rem;
    border-bottom: 2px solid #e2e8f0; background: #fff; position: sticky; top: 0;
  }
  thead th:first-child { text-align: left; width: 260px; }
  .plan-name { font-size: 1.1rem; font-weight: 700; margin-bottom: 4px; }
  .plan-price { font-size: 1.75rem; font-weight: 800; }
  .plan-price span { font-size: 0.85rem; font-weight: 500; color: #94a3b8; }
  .plan-period { font-size: 0.8rem; color: #94a3b8; }
  .popular { position: relative; }
  .popular::before {
    content: 'POPULAR'; position: absolute; top: 8px; right: 8px;
    background: #3b82f6; color: #fff; font-size: 0.65rem; font-weight: 700;
    padding: 3px 8px; border-radius: 4px; letter-spacing: 1px;
  }
  .popular-col { background: #f0f7ff; }
  tbody td {
    padding: 16px 20px; text-align: center; border-bottom: 1px solid #f1f5f9;
    font-size: 0.92rem;
  }
  tbody td:first-child { text-align: left; font-weight: 500; color: #334155; }
  tbody tr:hover { background: #fafbfc; }
  .check { color: #22c55e; font-size: 1.2rem; font-weight: 700; }
  .cross { color: #e2e8f0; font-size: 1.2rem; }
  .value { font-weight: 600; color: #334155; }
  .category-row td {
    background: #f8fafc; font-weight: 700; font-size: 0.78rem;
    text-transform: uppercase; letter-spacing: 1.5px; color: #64748b;
    padding: 12px 20px;
  }
  tfoot td { padding: 24px 20px; border-top: 2px solid #e2e8f0; }
  .btn {
    display: inline-block; padding: 10px 24px; border-radius: 10px;
    font-size: 0.88rem; font-weight: 700; text-decoration: none;
    transition: all 0.2s; cursor: pointer; border: none;
  }
  .btn-outline { background: #fff; color: #334155; border: 2px solid #e2e8f0; }
  .btn-outline:hover { border-color: #3b82f6; color: #3b82f6; }
  .btn-primary { background: #3b82f6; color: #fff; border: 2px solid #3b82f6; }
  .btn-primary:hover { background: #2563eb; }
  .btn-dark { background: #1e293b; color: #fff; border: 2px solid #1e293b; }
  .btn-dark:hover { background: #0f172a; }
</style>
</head>
<body>
  <div class="container">
    <div class="header">
      <h1>Choose Your Plan</h1>
      <p>Compare features and find the right fit for your team.</p>
    </div>
    <div class="table-wrap">
      <table>
        <thead>
          <tr>
            <th></th>
            <th>
              <div class="plan-name">Starter</div>
              <div class="plan-price">$0 <span>/mo</span></div>
              <div class="plan-period">Free forever</div>
            </th>
            <th class="popular">
              <div class="plan-name">Pro</div>
              <div class="plan-price">$29 <span>/mo</span></div>
              <div class="plan-period">Billed annually</div>
            </th>
            <th>
              <div class="plan-name">Enterprise</div>
              <div class="plan-price">$99 <span>/mo</span></div>
              <div class="plan-period">Billed annually</div>
            </th>
          </tr>
        </thead>
        <tbody>
          <tr class="category-row"><td colspan="4">Core Features</td></tr>
          <tr><td>Projects</td><td class="value">3</td><td class="popular-col value">Unlimited</td><td class="value">Unlimited</td></tr>
          <tr><td>Team Members</td><td class="value">1</td><td class="popular-col value">10</td><td class="value">Unlimited</td></tr>
          <tr><td>Storage</td><td class="value">1 GB</td><td class="popular-col value">50 GB</td><td class="value">500 GB</td></tr>
          <tr><td>API Requests</td><td class="value">1K/day</td><td class="popular-col value">100K/day</td><td class="value">Unlimited</td></tr>
          <tr class="category-row"><td colspan="4">Collaboration</td></tr>
          <tr><td>Real-time Editing</td><td class="check">&#x2713;</td><td class="popular-col check">&#x2713;</td><td class="check">&#x2713;</td></tr>
          <tr><td>Comments &amp; Threads</td><td class="cross">&#x2717;</td><td class="popular-col check">&#x2713;</td><td class="check">&#x2713;</td></tr>
          <tr><td>Version History</td><td class="value">7 days</td><td class="popular-col value">90 days</td><td class="value">Unlimited</td></tr>
          <tr><td>Guest Access</td><td class="cross">&#x2717;</td><td class="popular-col check">&#x2713;</td><td class="check">&#x2713;</td></tr>
          <tr class="category-row"><td colspan="4">Security &amp; Compliance</td></tr>
          <tr><td>SSO / SAML</td><td class="cross">&#x2717;</td><td class="popular-col cross">&#x2717;</td><td class="check">&#x2713;</td></tr>
          <tr><td>Audit Logs</td><td class="cross">&#x2717;</td><td class="popular-col check">&#x2713;</td><td class="check">&#x2713;</td></tr>
          <tr><td>Custom Roles</td><td class="cross">&#x2717;</td><td class="popular-col cross">&#x2717;</td><td class="check">&#x2713;</td></tr>
          <tr><td>SOC 2 Report</td><td class="cross">&#x2717;</td><td class="popular-col cross">&#x2717;</td><td class="check">&#x2713;</td></tr>
          <tr class="category-row"><td colspan="4">Support</td></tr>
          <tr><td>Community Forum</td><td class="check">&#x2713;</td><td class="popular-col check">&#x2713;</td><td class="check">&#x2713;</td></tr>
          <tr><td>Email Support</td><td class="cross">&#x2717;</td><td class="popular-col check">&#x2713;</td><td class="check">&#x2713;</td></tr>
          <tr><td>Dedicated Manager</td><td class="cross">&#x2717;</td><td class="popular-col cross">&#x2717;</td><td class="check">&#x2713;</td></tr>
          <tr><td>SLA Guarantee</td><td class="cross">&#x2717;</td><td class="popular-col value">99.9%</td><td class="value">99.99%</td></tr>
        </tbody>
        <tfoot>
          <tr>
            <td></td>
            <td><button class="btn btn-outline">Get Started</button></td>
            <td class="popular-col"><button class="btn btn-primary">Start Free Trial</button></td>
            <td><button class="btn btn-dark">Contact Sales</button></td>
          </tr>
        </tfoot>
      </table>
    </div>
  </div>
</body>
</html>"##;

#[no_mangle]
pub extern "C" fn x402_handle() {
    host_log(1, "comparison_table: serving feature comparison");
    respond(200, BODY, "text/html; charset=utf-8");
}
