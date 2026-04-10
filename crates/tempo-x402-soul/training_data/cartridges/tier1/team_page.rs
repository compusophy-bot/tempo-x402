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
<title>Our Team</title>
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); min-height: 100vh; padding: 60px 20px; }
  .container { max-width: 1000px; margin: 0 auto; }
  h1 { text-align: center; font-size: 2.5rem; color: #fff; margin-bottom: 8px; }
  .subtitle { text-align: center; color: rgba(255,255,255,0.8); font-size: 1.1rem; margin-bottom: 52px; }
  .team-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 28px; }
  .member { background: #fff; border-radius: 16px; padding: 32px 20px; text-align: center; box-shadow: 0 10px 40px rgba(0,0,0,0.15); transition: transform 0.25s; }
  .member:hover { transform: translateY(-6px); }
  .avatar { width: 96px; height: 96px; border-radius: 50%; margin: 0 auto 18px; display: flex; align-items: center; justify-content: center; font-size: 2.2rem; font-weight: 700; color: #fff; }
  .av-blue { background: linear-gradient(135deg, #6366f1, #818cf8); }
  .av-green { background: linear-gradient(135deg, #10b981, #34d399); }
  .av-orange { background: linear-gradient(135deg, #f59e0b, #fbbf24); }
  .av-pink { background: linear-gradient(135deg, #ec4899, #f472b6); }
  .av-teal { background: linear-gradient(135deg, #14b8a6, #2dd4bf); }
  .av-red { background: linear-gradient(135deg, #ef4444, #f87171); }
  .name { font-size: 1.15rem; font-weight: 700; color: #1a1a2e; margin-bottom: 4px; }
  .role { font-size: 0.9rem; color: #6366f1; font-weight: 600; margin-bottom: 12px; }
  .bio { font-size: 0.85rem; color: #6b7280; line-height: 1.5; }
  .socials { margin-top: 16px; display: flex; justify-content: center; gap: 12px; }
  .socials a { display: inline-block; width: 32px; height: 32px; border-radius: 50%; background: #f3f4f6; color: #6b7280; text-decoration: none; line-height: 32px; font-size: 0.8rem; transition: background 0.2s; }
  .socials a:hover { background: #6366f1; color: #fff; }
</style>
</head>
<body>
  <div class="container">
    <h1>Meet the Team</h1>
    <p class="subtitle">The people behind the product</p>
    <div class="team-grid">
      <div class="member">
        <div class="avatar av-blue">AK</div>
        <div class="name">Alice Kim</div>
        <div class="role">CEO &amp; Co-Founder</div>
        <p class="bio">Former ML lead at DeepMind. 12 years building intelligent systems.</p>
        <div class="socials"><a href="#">in</a><a href="#">tw</a></div>
      </div>
      <div class="member">
        <div class="avatar av-green">MR</div>
        <div class="name">Marcus Rivera</div>
        <div class="role">CTO &amp; Co-Founder</div>
        <p class="bio">Systems architect. Rust evangelist. Previously core infra at Cloudflare.</p>
        <div class="socials"><a href="#">in</a><a href="#">gh</a></div>
      </div>
      <div class="member">
        <div class="avatar av-orange">SP</div>
        <div class="name">Sarah Patel</div>
        <div class="role">Head of Design</div>
        <p class="bio">Pixel-perfect obsessive. Led design systems at Stripe and Figma.</p>
        <div class="socials"><a href="#">in</a><a href="#">dr</a></div>
      </div>
      <div class="member">
        <div class="avatar av-pink">JC</div>
        <div class="name">James Chen</div>
        <div class="role">Lead Engineer</div>
        <p class="bio">Full-stack polyglot. Open source contributor. Loves compilers.</p>
        <div class="socials"><a href="#">gh</a><a href="#">tw</a></div>
      </div>
      <div class="member">
        <div class="avatar av-teal">LO</div>
        <div class="name">Lena Olsson</div>
        <div class="role">Product Manager</div>
        <p class="bio">Data-driven PM with 8 years shipping B2B SaaS at scale.</p>
        <div class="socials"><a href="#">in</a><a href="#">tw</a></div>
      </div>
      <div class="member">
        <div class="avatar av-red">DW</div>
        <div class="name">David Wright</div>
        <div class="role">DevOps Lead</div>
        <p class="bio">Infrastructure whisperer. Kubernetes, Terraform, and bare metal.</p>
        <div class="socials"><a href="#">gh</a><a href="#">in</a></div>
      </div>
    </div>
  </div>
</body>
</html>"#;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    host_log(1, "serving team page");
    respond(200, PAGE, "text/html; charset=utf-8");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
