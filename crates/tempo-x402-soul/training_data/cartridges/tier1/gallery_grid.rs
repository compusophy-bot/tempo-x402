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
<title>Gallery Grid</title>
<style>
  *, *::before, *::after { margin: 0; padding: 0; box-sizing: border-box; }
  body { font-family: 'Segoe UI', system-ui, sans-serif; background: #111; color: #fff; padding: 40px 20px; }
  .container { max-width: 1100px; margin: 0 auto; }
  .header { text-align: center; margin-bottom: 24px; }
  .header h1 { font-size: 2rem; font-weight: 800; margin-bottom: 8px; }
  .header p { color: #888; font-size: 0.95rem; }
  .filters {
    display: flex; justify-content: center; gap: 8px; margin-bottom: 36px; flex-wrap: wrap;
  }
  .filter-btn {
    padding: 8px 20px; border-radius: 999px; border: 1px solid #333;
    background: transparent; color: #999; font-size: 0.85rem; font-weight: 600;
    cursor: pointer; transition: all 0.2s;
  }
  .filter-btn.active { background: #fff; color: #111; border-color: #fff; }
  .filter-btn:hover { border-color: #666; color: #fff; }
  .gallery {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    grid-auto-rows: 200px;
    gap: 8px;
  }
  .item {
    border-radius: 12px; position: relative; overflow: hidden;
    cursor: pointer; transition: transform 0.3s;
  }
  .item:hover { transform: scale(1.02); z-index: 1; }
  .item:hover .overlay { opacity: 1; }
  .item.tall { grid-row: span 2; }
  .item.wide { grid-column: span 2; }
  .item.big { grid-column: span 2; grid-row: span 2; }
  .color-fill { width: 100%; height: 100%; }
  .c1 { background: linear-gradient(135deg, #667eea, #764ba2); }
  .c2 { background: linear-gradient(135deg, #f093fb, #f5576c); }
  .c3 { background: linear-gradient(135deg, #4facfe, #00f2fe); }
  .c4 { background: linear-gradient(135deg, #43e97b, #38f9d7); }
  .c5 { background: linear-gradient(135deg, #fa709a, #fee140); }
  .c6 { background: linear-gradient(135deg, #a18cd1, #fbc2eb); }
  .c7 { background: linear-gradient(135deg, #ffecd2, #fcb69f); }
  .c8 { background: linear-gradient(135deg, #ff9a9e, #fad0c4); }
  .c9 { background: linear-gradient(135deg, #a1c4fd, #c2e9fb); }
  .c10 { background: linear-gradient(135deg, #d4fc79, #96e6a1); }
  .c11 { background: linear-gradient(135deg, #84fab0, #8fd3f4); }
  .c12 { background: linear-gradient(135deg, #cfd9df, #e2ebf0); }
  .overlay {
    position: absolute; inset: 0;
    background: linear-gradient(transparent 40%, rgba(0,0,0,0.7));
    opacity: 0; transition: opacity 0.3s;
    display: flex; flex-direction: column; justify-content: flex-end;
    padding: 20px;
  }
  .overlay .title { font-size: 1rem; font-weight: 700; margin-bottom: 4px; }
  .overlay .meta { font-size: 0.8rem; color: rgba(255,255,255,0.7); }
  .overlay .actions { display: flex; gap: 8px; margin-top: 8px; }
  .overlay .action-btn {
    width: 32px; height: 32px; border-radius: 8px;
    background: rgba(255,255,255,0.15); border: none;
    color: #fff; font-size: 0.9rem; cursor: pointer;
    display: flex; align-items: center; justify-content: center;
    backdrop-filter: blur(4px); transition: background 0.2s;
  }
  .overlay .action-btn:hover { background: rgba(255,255,255,0.3); }
  .stats { display: flex; justify-content: center; gap: 48px; margin-top: 36px; padding-top: 24px; border-top: 1px solid #222; }
  .stat { text-align: center; }
  .stat-value { font-size: 1.5rem; font-weight: 800; }
  .stat-label { font-size: 0.78rem; color: #666; margin-top: 2px; }
</style>
</head>
<body>
  <div class="container">
    <div class="header">
      <h1>Gallery</h1>
      <p>A curated collection of gradient compositions</p>
    </div>
    <div class="filters">
      <button class="filter-btn active">All</button>
      <button class="filter-btn">Warm</button>
      <button class="filter-btn">Cool</button>
      <button class="filter-btn">Pastel</button>
      <button class="filter-btn">Vivid</button>
    </div>
    <div class="gallery">
      <div class="item big"><div class="color-fill c1"></div><div class="overlay"><span class="title">Cosmic Drift</span><span class="meta">Vivid &middot; 3840x2160</span><div class="actions"><button class="action-btn">&#x2661;</button><button class="action-btn">&#x2913;</button></div></div></div>
      <div class="item"><div class="color-fill c2"></div><div class="overlay"><span class="title">Sunset Bloom</span><span class="meta">Warm</span></div></div>
      <div class="item"><div class="color-fill c3"></div><div class="overlay"><span class="title">Ocean Breeze</span><span class="meta">Cool</span></div></div>
      <div class="item tall"><div class="color-fill c4"></div><div class="overlay"><span class="title">Emerald Flow</span><span class="meta">Cool</span></div></div>
      <div class="item"><div class="color-fill c5"></div><div class="overlay"><span class="title">Citrus Splash</span><span class="meta">Warm</span></div></div>
      <div class="item"><div class="color-fill c6"></div><div class="overlay"><span class="title">Lavender Mist</span><span class="meta">Pastel</span></div></div>
      <div class="item wide"><div class="color-fill c7"></div><div class="overlay"><span class="title">Peach Horizon</span><span class="meta">Pastel &middot; 3840x1080</span></div></div>
      <div class="item"><div class="color-fill c8"></div><div class="overlay"><span class="title">Rose Quartz</span><span class="meta">Pastel</span></div></div>
      <div class="item"><div class="color-fill c9"></div><div class="overlay"><span class="title">Arctic Light</span><span class="meta">Cool</span></div></div>
      <div class="item"><div class="color-fill c10"></div><div class="overlay"><span class="title">Spring Meadow</span><span class="meta">Vivid</span></div></div>
      <div class="item"><div class="color-fill c11"></div><div class="overlay"><span class="title">Aqua Silk</span><span class="meta">Cool</span></div></div>
      <div class="item"><div class="color-fill c12"></div><div class="overlay"><span class="title">Silver Fog</span><span class="meta">Pastel</span></div></div>
    </div>
    <div class="stats">
      <div class="stat"><div class="stat-value">12</div><div class="stat-label">Items</div></div>
      <div class="stat"><div class="stat-value">4</div><div class="stat-label">Categories</div></div>
      <div class="stat"><div class="stat-value">48K</div><div class="stat-label">Downloads</div></div>
    </div>
  </div>
</body>
</html>"##;

#[no_mangle]
pub extern "C" fn x402_handle() {
    host_log(1, "gallery_grid: serving gallery");
    respond(200, BODY, "text/html; charset=utf-8");
}
