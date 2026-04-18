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
<title>Documentation - API Reference</title>
<style>
  *, *::before, *::after { margin: 0; padding: 0; box-sizing: border-box; }
  body { font-family: 'Segoe UI', system-ui, sans-serif; background: #f8fafc; color: #1e293b; }
  .layout { display: grid; grid-template-columns: 260px 1fr; min-height: 100vh; }
  .sidebar {
    background: #fff; border-right: 1px solid #e2e8f0; padding: 24px 0;
    position: sticky; top: 0; height: 100vh; overflow-y: auto;
  }
  .sidebar-logo { padding: 0 24px 24px; font-size: 1.15rem; font-weight: 800; color: #0f172a; border-bottom: 1px solid #f1f5f9; }
  .sidebar-section { padding: 16px 0; }
  .sidebar-heading {
    padding: 0 24px; font-size: 0.72rem; text-transform: uppercase;
    letter-spacing: 1.5px; color: #94a3b8; font-weight: 700; margin-bottom: 8px;
  }
  .sidebar-link {
    display: block; padding: 8px 24px; font-size: 0.88rem; color: #64748b;
    text-decoration: none; transition: all 0.15s; border-left: 3px solid transparent;
  }
  .sidebar-link:hover { color: #1e293b; background: #f8fafc; }
  .sidebar-link.active { color: #3b82f6; background: #eff6ff; border-left-color: #3b82f6; font-weight: 600; }
  .sidebar-link .badge-sm {
    float: right; font-size: 0.68rem; padding: 1px 6px; border-radius: 4px;
    background: #dbeafe; color: #3b82f6; font-weight: 700;
  }
  .main { padding: 32px 48px; max-width: 860px; }
  .breadcrumbs {
    display: flex; align-items: center; gap: 8px;
    font-size: 0.85rem; margin-bottom: 32px;
  }
  .breadcrumbs a { color: #64748b; text-decoration: none; transition: color 0.15s; }
  .breadcrumbs a:hover { color: #3b82f6; }
  .breadcrumbs .sep { color: #cbd5e1; }
  .breadcrumbs .current { color: #1e293b; font-weight: 600; }
  .main h1 { font-size: 2rem; font-weight: 800; margin-bottom: 8px; }
  .main .subtitle { color: #64748b; font-size: 1rem; margin-bottom: 32px; line-height: 1.5; }
  .section { margin-bottom: 36px; }
  .section h2 { font-size: 1.25rem; font-weight: 700; margin-bottom: 12px; padding-bottom: 8px; border-bottom: 1px solid #e2e8f0; }
  .section p { font-size: 0.92rem; color: #475569; line-height: 1.7; margin-bottom: 12px; }
  .code-block {
    background: #0f172a; color: #e2e8f0; border-radius: 10px; padding: 20px;
    font-family: 'Cascadia Code', monospace; font-size: 0.85rem;
    line-height: 1.7; overflow-x: auto; margin: 16px 0;
  }
  .code-block .kw { color: #c084fc; }
  .code-block .str { color: #34d399; }
  .code-block .cm { color: #64748b; }
  .code-block .fn { color: #38bdf8; }
  .param-table { width: 100%; border-collapse: collapse; margin: 16px 0; font-size: 0.88rem; }
  .param-table th { text-align: left; padding: 10px 12px; background: #f1f5f9; font-weight: 700; font-size: 0.78rem; text-transform: uppercase; letter-spacing: 1px; color: #64748b; }
  .param-table td { padding: 10px 12px; border-bottom: 1px solid #f1f5f9; }
  .param-table code { background: #f1f5f9; padding: 2px 6px; border-radius: 4px; font-size: 0.82rem; color: #e11d48; }
  .type-badge { font-size: 0.75rem; padding: 2px 8px; border-radius: 4px; font-weight: 600; }
  .type-badge.str { background: #dcfce7; color: #16a34a; }
  .type-badge.num { background: #dbeafe; color: #2563eb; }
  .type-badge.bool { background: #fef3c7; color: #d97706; }
  .pagination { display: flex; justify-content: space-between; margin-top: 48px; padding-top: 24px; border-top: 1px solid #e2e8f0; }
  .page-link { font-size: 0.88rem; color: #3b82f6; text-decoration: none; font-weight: 600; }
  .page-link:hover { text-decoration: underline; }
  .page-link .label { font-size: 0.75rem; color: #94a3b8; display: block; font-weight: 500; }
</style>
</head>
<body>
  <div class="layout">
    <nav class="sidebar">
      <div class="sidebar-logo">DevDocs</div>
      <div class="sidebar-section">
        <div class="sidebar-heading">Getting Started</div>
        <a href="#" class="sidebar-link">Introduction</a>
        <a href="#" class="sidebar-link">Quick Start</a>
        <a href="#" class="sidebar-link">Installation</a>
      </div>
      <div class="sidebar-section">
        <div class="sidebar-heading">API Reference</div>
        <a href="#" class="sidebar-link active">Authentication <span class="badge-sm">JWT</span></a>
        <a href="#" class="sidebar-link">Endpoints</a>
        <a href="#" class="sidebar-link">Error Codes</a>
        <a href="#" class="sidebar-link">Rate Limiting</a>
        <a href="#" class="sidebar-link">Webhooks</a>
      </div>
      <div class="sidebar-section">
        <div class="sidebar-heading">Guides</div>
        <a href="#" class="sidebar-link">Pagination</a>
        <a href="#" class="sidebar-link">Filtering</a>
        <a href="#" class="sidebar-link">Batch Operations</a>
      </div>
      <div class="sidebar-section">
        <div class="sidebar-heading">SDKs</div>
        <a href="#" class="sidebar-link">Python</a>
        <a href="#" class="sidebar-link">TypeScript</a>
        <a href="#" class="sidebar-link">Rust</a>
        <a href="#" class="sidebar-link">Go</a>
      </div>
    </nav>
    <main class="main">
      <div class="breadcrumbs">
        <a href="#">Home</a><span class="sep">/</span>
        <a href="#">API Reference</a><span class="sep">/</span>
        <span class="current">Authentication</span>
      </div>
      <h1>Authentication</h1>
      <p class="subtitle">Learn how to authenticate API requests using JWT tokens. All endpoints require a valid Bearer token in the Authorization header.</p>
      <div class="section">
        <h2>Overview</h2>
        <p>The API uses JSON Web Tokens (JWT) for authentication. Obtain a token by sending your credentials to the auth endpoint, then include it in all subsequent requests.</p>
        <div class="code-block">
          <span class="cm">// Request a token</span><br>
          <span class="kw">POST</span> /api/v1/auth/token<br><br>
          <span class="cm">// Use the token</span><br>
          <span class="kw">GET</span> /api/v1/resources<br>
          Authorization: Bearer <span class="str">&lt;your-token&gt;</span>
        </div>
      </div>
      <div class="section">
        <h2>Parameters</h2>
        <table class="param-table">
          <thead><tr><th>Name</th><th>Type</th><th>Required</th><th>Description</th></tr></thead>
          <tbody>
            <tr><td><code>client_id</code></td><td><span class="type-badge str">string</span></td><td>Yes</td><td>Your application client ID</td></tr>
            <tr><td><code>client_secret</code></td><td><span class="type-badge str">string</span></td><td>Yes</td><td>Your application secret key</td></tr>
            <tr><td><code>grant_type</code></td><td><span class="type-badge str">string</span></td><td>Yes</td><td>Must be "client_credentials"</td></tr>
            <tr><td><code>scope</code></td><td><span class="type-badge str">string</span></td><td>No</td><td>Space-separated list of scopes</td></tr>
            <tr><td><code>expires_in</code></td><td><span class="type-badge num">number</span></td><td>No</td><td>Token TTL in seconds (default: 3600)</td></tr>
          </tbody>
        </table>
      </div>
      <div class="section">
        <h2>Response</h2>
        <div class="code-block">
          {<br>
          &nbsp;&nbsp;<span class="str">"access_token"</span>: <span class="str">"eyJhbGciOi..."</span>,<br>
          &nbsp;&nbsp;<span class="str">"token_type"</span>: <span class="str">"Bearer"</span>,<br>
          &nbsp;&nbsp;<span class="str">"expires_in"</span>: <span class="fn">3600</span>,<br>
          &nbsp;&nbsp;<span class="str">"scope"</span>: <span class="str">"read write"</span><br>
          }
        </div>
      </div>
      <div class="pagination">
        <a href="#" class="page-link"><span class="label">Previous</span>Installation</a>
        <a href="#" class="page-link" style="text-align:right"><span class="label">Next</span>Endpoints</a>
      </div>
    </main>
  </div>
</body>
</html>"##;

#[no_mangle]
pub extern "C" fn x402_handle() {
    host_log(1, "breadcrumb_nav: serving documentation with breadcrumbs");
    respond(200, BODY, "text/html; charset=utf-8");
}
