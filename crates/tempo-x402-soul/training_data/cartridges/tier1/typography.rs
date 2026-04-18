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
<title>Typography Specimen</title>
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #fafbfc; color: #1a1a2e; }
  .hero { background: #1a1a2e; color: #fff; padding: 72px 24px; text-align: center; }
  .hero h1 { font-size: 3.5rem; font-weight: 800; letter-spacing: -0.03em; margin-bottom: 12px; }
  .hero p { color: rgba(255,255,255,0.6); font-size: 1.1rem; }
  .content { max-width: 780px; margin: 0 auto; padding: 56px 24px 80px; }
  .section { margin-bottom: 48px; }
  .section-label { font-size: 0.75rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.12em; color: #6366f1; margin-bottom: 20px; padding-bottom: 8px; border-bottom: 2px solid #e5e7eb; }

  /* Headings */
  .headings h1 { font-size: 2.8rem; font-weight: 800; line-height: 1.15; margin-bottom: 8px; letter-spacing: -0.02em; }
  .headings h2 { font-size: 2.1rem; font-weight: 700; line-height: 1.2; margin-bottom: 8px; }
  .headings h3 { font-size: 1.6rem; font-weight: 600; line-height: 1.3; margin-bottom: 8px; }
  .headings h4 { font-size: 1.25rem; font-weight: 600; line-height: 1.4; margin-bottom: 8px; }
  .headings h5 { font-size: 1rem; font-weight: 600; line-height: 1.5; margin-bottom: 8px; }
  .headings h6 { font-size: 0.875rem; font-weight: 600; line-height: 1.5; margin-bottom: 8px; text-transform: uppercase; letter-spacing: 0.05em; color: #6b7280; }
  .size-label { font-size: 0.75rem; color: #9ca3af; margin-bottom: 16px; display: block; }

  /* Body text */
  .body-text p { font-size: 1rem; line-height: 1.75; color: #374151; margin-bottom: 16px; }
  .body-text .lead { font-size: 1.2rem; line-height: 1.7; color: #1a1a2e; font-weight: 400; }
  .body-text .small { font-size: 0.875rem; color: #6b7280; }
  .body-text .tiny { font-size: 0.75rem; color: #9ca3af; letter-spacing: 0.02em; }

  /* Weights */
  .weights { display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 16px; }
  .weight-sample { background: #fff; border: 1px solid #e5e7eb; border-radius: 10px; padding: 20px; }
  .weight-sample .preview { font-size: 1.6rem; margin-bottom: 8px; }
  .weight-sample .meta { font-size: 0.75rem; color: #9ca3af; }
  .w100 { font-weight: 100; } .w300 { font-weight: 300; } .w400 { font-weight: 400; }
  .w500 { font-weight: 500; } .w600 { font-weight: 600; } .w700 { font-weight: 700; }
  .w800 { font-weight: 800; } .w900 { font-weight: 900; }

  /* Code */
  code { font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', Consolas, monospace; background: #f1f5f9; padding: 2px 6px; border-radius: 4px; font-size: 0.9em; color: #e11d48; }
  pre { font-family: 'SF Mono', 'Fira Code', Consolas, monospace; background: #1e293b; color: #e2e8f0; padding: 24px; border-radius: 12px; font-size: 0.9rem; line-height: 1.6; overflow-x: auto; }
  pre .kw { color: #c084fc; } pre .fn { color: #67e8f9; } pre .str { color: #86efac; }
  pre .cm { color: #64748b; font-style: italic; } pre .num { color: #fbbf24; }

  /* Lists */
  ul, ol { margin: 0 0 16px 24px; line-height: 1.8; color: #374151; }
  li { margin-bottom: 4px; }

  /* Blockquote */
  blockquote { border-left: 4px solid #6366f1; padding: 16px 24px; margin: 20px 0; background: #f5f3ff; border-radius: 0 8px 8px 0; }
  blockquote p { color: #4338ca; font-style: italic; font-size: 1.05rem; line-height: 1.6; margin: 0; }
  blockquote cite { display: block; margin-top: 8px; font-size: 0.85rem; color: #7c3aed; font-style: normal; }

  /* Table */
  table { width: 100%; border-collapse: collapse; margin: 16px 0; }
  th, td { text-align: left; padding: 12px 16px; border-bottom: 1px solid #e5e7eb; font-size: 0.9rem; }
  th { font-weight: 700; color: #1a1a2e; background: #f9fafb; }
  td { color: #4b5563; }
</style>
</head>
<body>
<div class="hero">
  <h1>Typography Specimen</h1>
  <p>A showcase of type scales, weights, and text styles</p>
</div>
<div class="content">
  <div class="section headings">
    <div class="section-label">Headings</div>
    <h1>Heading One</h1><span class="size-label">2.8rem / 800</span>
    <h2>Heading Two</h2><span class="size-label">2.1rem / 700</span>
    <h3>Heading Three</h3><span class="size-label">1.6rem / 600</span>
    <h4>Heading Four</h4><span class="size-label">1.25rem / 600</span>
    <h5>Heading Five</h5><span class="size-label">1rem / 600</span>
    <h6>Heading Six</h6><span class="size-label">0.875rem / 600</span>
  </div>

  <div class="section body-text">
    <div class="section-label">Body Text</div>
    <p class="lead">Lead paragraph: The quick brown fox jumps over the lazy dog. This larger introductory text draws the reader in and sets the tone for the content that follows.</p>
    <p>Regular body text at 1rem. Typography is the art and technique of arranging type to make written language legible, readable, and appealing when displayed. The arrangement of type involves selecting typefaces, point sizes, line lengths, line spacing, and letter spacing.</p>
    <p class="small">Small text at 0.875rem for secondary information, captions, and supporting details that should be present but not prominent.</p>
    <p class="tiny">Tiny text at 0.75rem for legal disclaimers, timestamps, and metadata.</p>
  </div>

  <div class="section">
    <div class="section-label">Font Weights</div>
    <div class="weights">
      <div class="weight-sample"><div class="preview w100">Aa</div><div class="meta">100 Thin</div></div>
      <div class="weight-sample"><div class="preview w300">Aa</div><div class="meta">300 Light</div></div>
      <div class="weight-sample"><div class="preview w400">Aa</div><div class="meta">400 Regular</div></div>
      <div class="weight-sample"><div class="preview w500">Aa</div><div class="meta">500 Medium</div></div>
      <div class="weight-sample"><div class="preview w600">Aa</div><div class="meta">600 Semibold</div></div>
      <div class="weight-sample"><div class="preview w700">Aa</div><div class="meta">700 Bold</div></div>
      <div class="weight-sample"><div class="preview w800">Aa</div><div class="meta">800 Extrabold</div></div>
      <div class="weight-sample"><div class="preview w900">Aa</div><div class="meta">900 Black</div></div>
    </div>
  </div>

  <div class="section">
    <div class="section-label">Code</div>
    <p style="margin-bottom:12px;color:#374151;">Inline: Use <code>cargo build --release</code> to compile.</p>
    <pre><span class="cm">// Fibonacci sequence</span>
<span class="kw">fn</span> <span class="fn">fibonacci</span>(n: <span class="kw">u64</span>) -> <span class="kw">u64</span> {
    <span class="kw">match</span> n {
        <span class="num">0</span> => <span class="num">0</span>,
        <span class="num">1</span> => <span class="num">1</span>,
        _ => <span class="fn">fibonacci</span>(n - <span class="num">1</span>) + <span class="fn">fibonacci</span>(n - <span class="num">2</span>),
    }
}</pre>
  </div>

  <div class="section">
    <div class="section-label">Lists</div>
    <ul>
      <li>Unordered list item one</li>
      <li>Unordered list item two with longer text that might wrap to the next line depending on viewport width</li>
      <li>Unordered list item three</li>
    </ul>
    <ol>
      <li>First ordered item</li>
      <li>Second ordered item</li>
      <li>Third ordered item</li>
    </ol>
  </div>

  <div class="section">
    <div class="section-label">Blockquote</div>
    <blockquote>
      <p>Good typography is invisible. Bad typography is everywhere.</p>
      <cite>Oliver Reichenstein</cite>
    </blockquote>
  </div>

  <div class="section">
    <div class="section-label">Table</div>
    <table>
      <tr><th>Property</th><th>Value</th><th>Usage</th></tr>
      <tr><td>Font Size</td><td>16px (1rem)</td><td>Body text baseline</td></tr>
      <tr><td>Line Height</td><td>1.75</td><td>Optimal reading comfort</td></tr>
      <tr><td>Measure</td><td>65-75ch</td><td>Ideal line length</td></tr>
      <tr><td>Scale Ratio</td><td>1.25 (major third)</td><td>Heading progression</td></tr>
    </table>
  </div>
</div>
</body>
</html>"#;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    host_log(1, "serving typography specimen");
    respond(200, PAGE, "text/html; charset=utf-8");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
