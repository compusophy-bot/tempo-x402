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
<title>Product Card</title>
<style>
  *, *::before, *::after { margin: 0; padding: 0; box-sizing: border-box; }
  body {
    font-family: 'Segoe UI', system-ui, sans-serif;
    background: #f1f5f9; min-height: 100vh;
    display: flex; align-items: center; justify-content: center; padding: 40px 20px;
  }
  .card {
    width: 380px; background: #fff; border-radius: 16px;
    box-shadow: 0 4px 24px rgba(0,0,0,0.08); overflow: hidden;
    transition: transform 0.3s, box-shadow 0.3s;
  }
  .card:hover { transform: translateY(-6px); box-shadow: 0 16px 48px rgba(0,0,0,0.14); }
  .image-placeholder {
    width: 100%; height: 320px;
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    display: flex; align-items: center; justify-content: center;
    position: relative; overflow: hidden;
  }
  .image-placeholder::after {
    content: ''; position: absolute; top: -50%; left: -50%; width: 200%; height: 200%;
    background: radial-gradient(circle, rgba(255,255,255,0.1) 0%, transparent 60%);
    animation: shimmer 3s ease-in-out infinite;
  }
  @keyframes shimmer { 0%,100% { transform: translate(0,0); } 50% { transform: translate(10%,10%); } }
  .image-icon { font-size: 64px; z-index: 1; }
  .badge {
    position: absolute; top: 16px; left: 16px; z-index: 2;
    background: #ef4444; color: #fff; padding: 4px 12px;
    border-radius: 8px; font-size: 0.8rem; font-weight: 700;
    letter-spacing: 0.5px;
  }
  .wishlist {
    position: absolute; top: 16px; right: 16px; z-index: 2;
    width: 40px; height: 40px; border-radius: 50%;
    background: rgba(255,255,255,0.9); border: none; cursor: pointer;
    display: flex; align-items: center; justify-content: center;
    font-size: 1.2rem; transition: background 0.2s;
  }
  .wishlist:hover { background: #fff; }
  .content { padding: 24px; }
  .category { font-size: 0.78rem; text-transform: uppercase; letter-spacing: 1.5px; color: #64748b; font-weight: 600; margin-bottom: 8px; }
  .name { font-size: 1.25rem; font-weight: 700; color: #0f172a; margin-bottom: 8px; }
  .description { font-size: 0.9rem; color: #64748b; line-height: 1.5; margin-bottom: 16px; }
  .rating { display: flex; align-items: center; gap: 8px; margin-bottom: 16px; }
  .stars { color: #f59e0b; font-size: 0.95rem; letter-spacing: 2px; }
  .review-count { font-size: 0.82rem; color: #94a3b8; }
  .price-row { display: flex; align-items: baseline; gap: 12px; margin-bottom: 20px; }
  .price { font-size: 1.75rem; font-weight: 800; color: #0f172a; }
  .original-price { font-size: 1rem; color: #94a3b8; text-decoration: line-through; }
  .discount { font-size: 0.85rem; color: #22c55e; font-weight: 600; }
  .actions { display: flex; gap: 12px; }
  .btn-primary {
    flex: 1; padding: 14px; border: none; border-radius: 12px;
    background: linear-gradient(135deg, #667eea, #764ba2);
    color: #fff; font-size: 0.95rem; font-weight: 700;
    cursor: pointer; transition: opacity 0.2s;
  }
  .btn-primary:hover { opacity: 0.9; }
  .btn-secondary {
    padding: 14px 20px; border: 2px solid #e2e8f0; border-radius: 12px;
    background: #fff; color: #334155; font-size: 0.95rem; font-weight: 600;
    cursor: pointer; transition: border-color 0.2s;
  }
  .btn-secondary:hover { border-color: #667eea; }
  .sizes { display: flex; gap: 8px; margin-bottom: 20px; }
  .size {
    width: 40px; height: 40px; border-radius: 10px;
    border: 2px solid #e2e8f0; background: #fff;
    display: flex; align-items: center; justify-content: center;
    font-size: 0.8rem; font-weight: 600; color: #334155; cursor: pointer;
  }
  .size.active { border-color: #667eea; background: #eef2ff; color: #667eea; }
</style>
</head>
<body>
  <div class="card">
    <div class="image-placeholder">
      <span class="badge">-30%</span>
      <button class="wishlist">&#x2661;</button>
      <span class="image-icon">&#x1F45F;</span>
    </div>
    <div class="content">
      <div class="category">Running Shoes</div>
      <div class="name">Velocity Pro X</div>
      <div class="description">Ultra-lightweight carbon plate running shoe with responsive ZoomFoam midsole and breathable knit upper.</div>
      <div class="rating">
        <span class="stars">&#9733;&#9733;&#9733;&#9733;&#9734;</span>
        <span class="review-count">4.2 (1,847 reviews)</span>
      </div>
      <div class="sizes">
        <span class="size">8</span>
        <span class="size">9</span>
        <span class="size active">10</span>
        <span class="size">11</span>
        <span class="size">12</span>
      </div>
      <div class="price-row">
        <span class="price">$129.99</span>
        <span class="original-price">$189.99</span>
        <span class="discount">Save $60</span>
      </div>
      <div class="actions">
        <button class="btn-primary">Add to Cart</button>
        <button class="btn-secondary">&#x2764;</button>
      </div>
    </div>
  </div>
</body>
</html>"##;

#[no_mangle]
pub extern "C" fn x402_handle() {
    host_log(1, "product_card: serving product card");
    respond(200, BODY, "text/html; charset=utf-8");
}
