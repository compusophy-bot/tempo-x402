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
<title>Invoice #INV-2026-0047</title>
<style>
  *, *::before, *::after { margin: 0; padding: 0; box-sizing: border-box; }
  body { font-family: 'Segoe UI', system-ui, sans-serif; background: #f1f5f9; padding: 40px 20px; color: #1e293b; }
  .invoice {
    max-width: 800px; margin: 0 auto; background: #fff;
    border-radius: 12px; box-shadow: 0 2px 16px rgba(0,0,0,0.06);
    overflow: hidden;
  }
  .header {
    background: linear-gradient(135deg, #1e293b, #334155);
    color: #fff; padding: 40px; display: flex; justify-content: space-between; align-items: flex-start;
  }
  .logo { font-size: 1.75rem; font-weight: 800; letter-spacing: -0.5px; }
  .logo span { color: #38bdf8; }
  .invoice-label { text-align: right; }
  .invoice-label h2 { font-size: 1.4rem; font-weight: 700; margin-bottom: 4px; text-transform: uppercase; letter-spacing: 2px; }
  .invoice-label .num { font-size: 0.95rem; color: #94a3b8; }
  .body { padding: 40px; }
  .parties { display: flex; justify-content: space-between; margin-bottom: 40px; gap: 24px; flex-wrap: wrap; }
  .party h4 { font-size: 0.75rem; text-transform: uppercase; letter-spacing: 1.5px; color: #94a3b8; margin-bottom: 8px; font-weight: 600; }
  .party .name { font-size: 1.05rem; font-weight: 700; margin-bottom: 4px; }
  .party .detail { font-size: 0.88rem; color: #64748b; line-height: 1.6; }
  .meta { display: flex; gap: 32px; margin-bottom: 36px; flex-wrap: wrap; }
  .meta-item h4 { font-size: 0.72rem; text-transform: uppercase; letter-spacing: 1.5px; color: #94a3b8; font-weight: 600; margin-bottom: 4px; }
  .meta-item .value { font-size: 0.95rem; font-weight: 600; }
  .status { display: inline-block; padding: 4px 12px; border-radius: 6px; font-size: 0.78rem; font-weight: 700; }
  .status.paid { background: #dcfce7; color: #16a34a; }
  table { width: 100%; border-collapse: collapse; margin-bottom: 32px; }
  thead th {
    text-align: left; padding: 12px 16px; font-size: 0.75rem;
    text-transform: uppercase; letter-spacing: 1px; color: #64748b;
    border-bottom: 2px solid #e2e8f0; font-weight: 700;
  }
  thead th:last-child, thead th:nth-child(3), thead th:nth-child(4) { text-align: right; }
  tbody td {
    padding: 16px; border-bottom: 1px solid #f1f5f9;
    font-size: 0.92rem;
  }
  tbody td:last-child, tbody td:nth-child(3), tbody td:nth-child(4) { text-align: right; font-variant-numeric: tabular-nums; }
  tbody td .item-name { font-weight: 600; }
  tbody td .item-desc { font-size: 0.82rem; color: #94a3b8; margin-top: 2px; }
  .totals {
    display: flex; justify-content: flex-end;
  }
  .totals-table { width: 280px; }
  .totals-row { display: flex; justify-content: space-between; padding: 8px 0; font-size: 0.92rem; }
  .totals-row.subtotal { color: #64748b; }
  .totals-row.tax { color: #64748b; }
  .totals-row.total {
    border-top: 2px solid #1e293b; margin-top: 8px; padding-top: 12px;
    font-size: 1.15rem; font-weight: 800;
  }
  .footer {
    padding: 32px 40px; background: #f8fafc; border-top: 1px solid #e2e8f0;
    display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 16px;
  }
  .footer .notes { font-size: 0.85rem; color: #64748b; max-width: 400px; line-height: 1.5; }
  .footer .notes strong { color: #334155; }
  .pay-btn {
    padding: 12px 32px; background: #1e293b; color: #fff;
    border: none; border-radius: 10px; font-size: 0.95rem; font-weight: 700;
    cursor: pointer; transition: background 0.2s;
  }
  .pay-btn:hover { background: #0f172a; }
</style>
</head>
<body>
  <div class="invoice">
    <div class="header">
      <div class="logo">Acme<span>Labs</span></div>
      <div class="invoice-label">
        <h2>Invoice</h2>
        <div class="num">#INV-2026-0047</div>
      </div>
    </div>
    <div class="body">
      <div class="parties">
        <div class="party">
          <h4>From</h4>
          <div class="name">AcmeLabs Inc.</div>
          <div class="detail">123 Innovation Blvd<br>San Francisco, CA 94105<br>billing@acmelabs.io</div>
        </div>
        <div class="party">
          <h4>Bill To</h4>
          <div class="name">Nexus Corp.</div>
          <div class="detail">456 Enterprise Ave<br>New York, NY 10013<br>accounts@nexuscorp.com</div>
        </div>
      </div>
      <div class="meta">
        <div class="meta-item"><h4>Issue Date</h4><div class="value">Apr 1, 2026</div></div>
        <div class="meta-item"><h4>Due Date</h4><div class="value">Apr 30, 2026</div></div>
        <div class="meta-item"><h4>Status</h4><div class="value"><span class="status paid">PAID</span></div></div>
      </div>
      <table>
        <thead>
          <tr><th>Item</th><th>Qty</th><th>Rate</th><th>Amount</th></tr>
        </thead>
        <tbody>
          <tr>
            <td><div class="item-name">Platform License</div><div class="item-desc">Enterprise tier, annual</div></td>
            <td>1</td><td>$12,000.00</td><td>$12,000.00</td>
          </tr>
          <tr>
            <td><div class="item-name">Implementation Services</div><div class="item-desc">Setup, migration, training</div></td>
            <td>40 hrs</td><td>$200.00</td><td>$8,000.00</td>
          </tr>
          <tr>
            <td><div class="item-name">Priority Support</div><div class="item-desc">24/7 SLA, dedicated engineer</div></td>
            <td>12 mo</td><td>$500.00</td><td>$6,000.00</td>
          </tr>
          <tr>
            <td><div class="item-name">Custom Integration</div><div class="item-desc">Salesforce + HubSpot connectors</div></td>
            <td>1</td><td>$3,500.00</td><td>$3,500.00</td>
          </tr>
        </tbody>
      </table>
      <div class="totals">
        <div class="totals-table">
          <div class="totals-row subtotal"><span>Subtotal</span><span>$29,500.00</span></div>
          <div class="totals-row tax"><span>Tax (8.5%)</span><span>$2,507.50</span></div>
          <div class="totals-row total"><span>Total</span><span>$32,007.50</span></div>
        </div>
      </div>
    </div>
    <div class="footer">
      <div class="notes"><strong>Payment Terms:</strong> Net 30. Please include invoice number on payment. Wire transfer preferred.</div>
      <button class="pay-btn">Pay Now</button>
    </div>
  </div>
</body>
</html>"##;

#[no_mangle]
pub extern "C" fn x402_handle() {
    host_log(1, "invoice_template: serving invoice");
    respond(200, BODY, "text/html; charset=utf-8");
}
