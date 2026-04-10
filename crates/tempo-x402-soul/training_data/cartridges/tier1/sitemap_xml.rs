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

const SITEMAP: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"
        xmlns:image="http://www.google.com/schemas/sitemap-image/1.1">
  <url>
    <loc>https://www.example.com/</loc>
    <lastmod>2026-04-10</lastmod>
    <changefreq>daily</changefreq>
    <priority>1.0</priority>
  </url>
  <url>
    <loc>https://www.example.com/about</loc>
    <lastmod>2026-04-01</lastmod>
    <changefreq>monthly</changefreq>
    <priority>0.8</priority>
  </url>
  <url>
    <loc>https://www.example.com/pricing</loc>
    <lastmod>2026-04-05</lastmod>
    <changefreq>weekly</changefreq>
    <priority>0.9</priority>
  </url>
  <url>
    <loc>https://www.example.com/docs</loc>
    <lastmod>2026-04-10</lastmod>
    <changefreq>daily</changefreq>
    <priority>0.9</priority>
  </url>
  <url>
    <loc>https://www.example.com/docs/getting-started</loc>
    <lastmod>2026-04-08</lastmod>
    <changefreq>weekly</changefreq>
    <priority>0.8</priority>
  </url>
  <url>
    <loc>https://www.example.com/docs/api-reference</loc>
    <lastmod>2026-04-10</lastmod>
    <changefreq>daily</changefreq>
    <priority>0.8</priority>
  </url>
  <url>
    <loc>https://www.example.com/docs/authentication</loc>
    <lastmod>2026-04-03</lastmod>
    <changefreq>monthly</changefreq>
    <priority>0.7</priority>
  </url>
  <url>
    <loc>https://www.example.com/docs/webhooks</loc>
    <lastmod>2026-04-06</lastmod>
    <changefreq>weekly</changefreq>
    <priority>0.7</priority>
  </url>
  <url>
    <loc>https://www.example.com/blog</loc>
    <lastmod>2026-04-10</lastmod>
    <changefreq>daily</changefreq>
    <priority>0.7</priority>
  </url>
  <url>
    <loc>https://www.example.com/blog/introducing-v2-api</loc>
    <lastmod>2026-04-09</lastmod>
    <changefreq>monthly</changefreq>
    <priority>0.6</priority>
    <image:image>
      <image:loc>https://www.example.com/images/v2-api-hero.jpg</image:loc>
      <image:title>Introducing the V2 API</image:title>
    </image:image>
  </url>
  <url>
    <loc>https://www.example.com/blog/scaling-to-million-requests</loc>
    <lastmod>2026-04-02</lastmod>
    <changefreq>monthly</changefreq>
    <priority>0.6</priority>
  </url>
  <url>
    <loc>https://www.example.com/blog/security-best-practices</loc>
    <lastmod>2026-03-28</lastmod>
    <changefreq>monthly</changefreq>
    <priority>0.6</priority>
  </url>
  <url>
    <loc>https://www.example.com/careers</loc>
    <lastmod>2026-04-07</lastmod>
    <changefreq>weekly</changefreq>
    <priority>0.5</priority>
  </url>
  <url>
    <loc>https://www.example.com/contact</loc>
    <lastmod>2026-03-15</lastmod>
    <changefreq>monthly</changefreq>
    <priority>0.5</priority>
  </url>
  <url>
    <loc>https://www.example.com/terms</loc>
    <lastmod>2026-04-01</lastmod>
    <changefreq>yearly</changefreq>
    <priority>0.3</priority>
  </url>
  <url>
    <loc>https://www.example.com/privacy</loc>
    <lastmod>2026-04-01</lastmod>
    <changefreq>yearly</changefreq>
    <priority>0.3</priority>
  </url>
  <url>
    <loc>https://www.example.com/status</loc>
    <lastmod>2026-04-10</lastmod>
    <changefreq>always</changefreq>
    <priority>0.4</priority>
  </url>
</urlset>"#;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    host_log(1, "serving sitemap.xml");
    respond(200, SITEMAP, "application/xml");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
