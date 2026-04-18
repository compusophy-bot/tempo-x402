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
<title>CSS Effects Demo</title>
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; background: #0f0f23; color: #e2e8f0; padding: 48px 20px; }
  .container { max-width: 900px; margin: 0 auto; }
  h1 { text-align: center; font-size: 2.5rem; margin-bottom: 48px; background: linear-gradient(90deg, #6366f1, #ec4899, #f59e0b); -webkit-background-clip: text; -webkit-text-fill-color: transparent; background-clip: text; }
  h2 { font-size: 1.1rem; color: #94a3b8; margin-bottom: 16px; text-transform: uppercase; letter-spacing: 0.1em; }
  .section { margin-bottom: 48px; }

  /* Gradient boxes */
  .gradients { display: grid; grid-template-columns: repeat(3, 1fr); gap: 16px; }
  .grad { height: 120px; border-radius: 16px; display: flex; align-items: flex-end; padding: 12px 16px; font-size: 0.8rem; font-weight: 600; color: #fff; text-shadow: 0 1px 3px rgba(0,0,0,0.4); }
  .g1 { background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); }
  .g2 { background: linear-gradient(135deg, #f093fb 0%, #f5576c 100%); }
  .g3 { background: linear-gradient(135deg, #4facfe 0%, #00f2fe 100%); }
  .g4 { background: conic-gradient(from 45deg, #6366f1, #ec4899, #f59e0b, #10b981, #6366f1); }
  .g5 { background: radial-gradient(circle at 30% 30%, #fbbf24, #ef4444); }
  .g6 { background: linear-gradient(to right, #0f0c29, #302b63, #24243e); }

  /* Animated cards */
  .anim-row { display: flex; gap: 20px; flex-wrap: wrap; }
  .anim-card { width: 180px; height: 180px; border-radius: 16px; display: flex; align-items: center; justify-content: center; font-weight: 700; font-size: 0.85rem; text-align: center; }

  .pulse-card { background: #6366f1; animation: pulse 2s ease-in-out infinite; }
  @keyframes pulse { 0%, 100% { transform: scale(1); box-shadow: 0 0 0 0 rgba(99,102,241,0.5); } 50% { transform: scale(1.05); box-shadow: 0 0 30px 10px rgba(99,102,241,0.3); } }

  .spin-card { background: linear-gradient(135deg, #ec4899, #f59e0b); animation: spin-glow 3s linear infinite; }
  @keyframes spin-glow { 0% { box-shadow: 5px 0 20px #ec4899; } 25% { box-shadow: 0 5px 20px #f59e0b; } 50% { box-shadow: -5px 0 20px #ec4899; } 75% { box-shadow: 0 -5px 20px #f59e0b; } 100% { box-shadow: 5px 0 20px #ec4899; } }

  .bounce-card { background: #10b981; animation: bounce 1.5s ease infinite; }
  @keyframes bounce { 0%, 100% { transform: translateY(0); } 50% { transform: translateY(-20px); } }

  .shake-card { background: #ef4444; animation: shake 0.6s ease-in-out infinite; }
  @keyframes shake { 0%, 100% { transform: translateX(0); } 25% { transform: translateX(-5px) rotate(-1deg); } 75% { transform: translateX(5px) rotate(1deg); } }

  /* Shadow showcase */
  .shadows { display: flex; gap: 24px; flex-wrap: wrap; }
  .shadow-box { width: 160px; height: 100px; background: #1e293b; border-radius: 12px; display: flex; align-items: center; justify-content: center; font-size: 0.8rem; color: #94a3b8; }
  .s1 { box-shadow: 0 4px 6px -1px rgba(0,0,0,0.3); }
  .s2 { box-shadow: 0 10px 25px -5px rgba(99,102,241,0.4); }
  .s3 { box-shadow: 0 0 40px rgba(236,72,153,0.3), 0 0 80px rgba(236,72,153,0.1); }
  .s4 { box-shadow: inset 0 2px 10px rgba(0,0,0,0.5), 0 4px 15px rgba(0,0,0,0.3); }
  .s5 { box-shadow: 8px 8px 0 #6366f1; }

  /* Glass morphism */
  .glass-container { background: linear-gradient(135deg, #6366f1 0%, #ec4899 100%); border-radius: 20px; padding: 40px; position: relative; overflow: hidden; }
  .glass-container::before { content: ''; position: absolute; width: 200px; height: 200px; background: rgba(255,255,255,0.15); border-radius: 50%; top: -60px; right: -40px; }
  .glass-container::after { content: ''; position: absolute; width: 150px; height: 150px; background: rgba(255,255,255,0.1); border-radius: 50%; bottom: -40px; left: 30px; }
  .glass-card { background: rgba(255,255,255,0.12); backdrop-filter: blur(12px); -webkit-backdrop-filter: blur(12px); border: 1px solid rgba(255,255,255,0.2); border-radius: 16px; padding: 28px; color: #fff; position: relative; z-index: 1; }
  .glass-card h3 { font-size: 1.2rem; margin-bottom: 8px; }
  .glass-card p { font-size: 0.9rem; opacity: 0.85; line-height: 1.6; }

  /* Hover transforms */
  .hover-row { display: flex; gap: 16px; flex-wrap: wrap; }
  .hover-box { width: 140px; height: 90px; background: #1e293b; border-radius: 10px; display: flex; align-items: center; justify-content: center; font-size: 0.8rem; color: #94a3b8; transition: all 0.3s ease; cursor: pointer; border: 1px solid #334155; }
  .hover-box:hover { background: #334155; color: #fff; }
  .h-scale:hover { transform: scale(1.15); }
  .h-rotate:hover { transform: rotate(8deg); }
  .h-skew:hover { transform: skewX(-5deg); }
  .h-lift:hover { transform: translateY(-10px); box-shadow: 0 20px 40px rgba(0,0,0,0.3); }
  .h-border:hover { border-color: #6366f1; box-shadow: 0 0 0 3px rgba(99,102,241,0.3); }
</style>
</head>
<body>
<div class="container">
  <h1>CSS Effects Showcase</h1>

  <div class="section">
    <h2>Gradients</h2>
    <div class="gradients">
      <div class="grad g1">Linear Diagonal</div>
      <div class="grad g2">Pink Sunset</div>
      <div class="grad g3">Ocean Blue</div>
      <div class="grad g4">Conic Rainbow</div>
      <div class="grad g5">Radial Warm</div>
      <div class="grad g6">Dark Linear</div>
    </div>
  </div>

  <div class="section">
    <h2>Animations</h2>
    <div class="anim-row">
      <div class="anim-card pulse-card">Pulse</div>
      <div class="anim-card spin-card">Glow Orbit</div>
      <div class="anim-card bounce-card">Bounce</div>
      <div class="anim-card shake-card">Shake</div>
    </div>
  </div>

  <div class="section">
    <h2>Shadows</h2>
    <div class="shadows">
      <div class="shadow-box s1">Subtle</div>
      <div class="shadow-box s2">Colored</div>
      <div class="shadow-box s3">Neon Glow</div>
      <div class="shadow-box s4">Inset</div>
      <div class="shadow-box s5">Hard Offset</div>
    </div>
  </div>

  <div class="section">
    <h2>Glassmorphism</h2>
    <div class="glass-container">
      <div class="glass-card">
        <h3>Frosted Glass Effect</h3>
        <p>This card uses backdrop-filter blur with a semi-transparent background and subtle border to create the popular glassmorphism aesthetic. Works best over colorful backgrounds.</p>
      </div>
    </div>
  </div>

  <div class="section">
    <h2>Hover Transforms</h2>
    <div class="hover-row">
      <div class="hover-box h-scale">Scale Up</div>
      <div class="hover-box h-rotate">Rotate</div>
      <div class="hover-box h-skew">Skew</div>
      <div class="hover-box h-lift">Lift</div>
      <div class="hover-box h-border">Focus Ring</div>
    </div>
  </div>
</div>
</body>
</html>"#;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    host_log(1, "serving css demo page");
    respond(200, PAGE, "text/html; charset=utf-8");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
