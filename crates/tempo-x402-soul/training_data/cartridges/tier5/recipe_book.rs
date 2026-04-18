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

fn respond(status: i32, body: &str, ct: &str) { unsafe { response(status, body.as_ptr(), body.len() as i32, ct.as_ptr(), ct.len() as i32); } }
fn host_log(level: i32, msg: &str) { unsafe { log(level, msg.as_ptr(), msg.len() as i32); } }
fn kv_read(key: &str) -> Option<&'static str> { unsafe { let r = kv_get(key.as_ptr(), key.len() as i32); if r < 0 { return None; } let p = (r >> 32) as *const u8; let l = (r & 0xFFFFFFFF) as usize; core::str::from_utf8(core::slice::from_raw_parts(p, l)).ok() } }
fn kv_write(key: &str, val: &str) { unsafe { kv_set(key.as_ptr(), key.len() as i32, val.as_ptr(), val.len() as i32); } }
fn find_json_str<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let kb = key.as_bytes(); let jb = json.as_bytes(); let mut i = 0;
    while i + kb.len() + 3 < jb.len() { if jb[i] == b'"' { let s = i + 1; if s + kb.len() < jb.len() && &jb[s..s + kb.len()] == kb && jb[s + kb.len()] == b'"' { let mut j = s + kb.len() + 1; while j < jb.len() && (jb[j] == b':' || jb[j] == b' ') { j += 1; } if j < jb.len() && jb[j] == b'"' { let vs = j + 1; let mut ve = vs; while ve < jb.len() && jb[ve] != b'"' { ve += 1; } return core::str::from_utf8(&jb[vs..ve]).ok(); } } } i += 1; } None
}
static mut BUF: [u8; 65536] = [0u8; 65536];
struct W { pos: usize }
impl W {
    fn new() -> Self { Self { pos: 0 } }
    fn s(&mut self, s: &str) { let b = s.as_bytes(); unsafe { let e = (self.pos + b.len()).min(BUF.len()); BUF[self.pos..e].copy_from_slice(&b[..e - self.pos]); self.pos = e; } }
    fn n(&mut self, mut n: u32) { if n == 0 { self.s("0"); return; } let mut d = [0u8; 10]; let mut i = 0; while n > 0 { d[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; } while i > 0 { i -= 1; unsafe { if self.pos < BUF.len() { BUF[self.pos] = d[i]; self.pos += 1; } } } }
    fn out(&self) -> &str { unsafe { core::str::from_utf8_unchecked(&BUF[..self.pos]) } }
}
static mut SCRATCH: [u8; 131072] = [0u8; 131072];
#[no_mangle] pub extern "C" fn x402_alloc(size: i32) -> *mut u8 { unsafe { SCRATCH.as_mut_ptr() } }
fn parse_u32(s: &str) -> u32 { let mut n: u32 = 0; for &b in s.as_bytes() { if b >= b'0' && b <= b'9' { n = n * 10 + (b - b'0') as u32; } } n }
fn write_key(buf: &mut [u8], prefix: &[u8], num: u32) -> usize {
    let mut pos = 0; for &b in prefix { buf[pos] = b; pos += 1; }
    if num == 0 { buf[pos] = b'0'; return pos + 1; }
    let mut d = [0u8; 10]; let mut di = 0; let mut n = num;
    while n > 0 { d[di] = b'0' + (n % 10) as u8; n /= 10; di += 1; }
    while di > 0 { di -= 1; buf[pos] = d[di]; pos += 1; } pos
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(request_ptr, request_len as usize)) };
    let method = find_json_str(request, "method").unwrap_or("GET");
    let path = find_json_str(request, "path").unwrap_or("/");
    let body = find_json_str(request, "body").unwrap_or("");
    host_log(0, "recipe_book: handling request");

    if method == "POST" {
        if let Some(action) = find_json_str(body, "action") {
            if action == "add_recipe" {
                let title = find_json_str(body, "title").unwrap_or("");
                let category = find_json_str(body, "category").unwrap_or("other");
                let servings = find_json_str(body, "servings").unwrap_or("4");
                let prep_time = find_json_str(body, "prep_time").unwrap_or("30");
                let ingredients = find_json_str(body, "ingredients").unwrap_or("");
                let instructions = find_json_str(body, "instructions").unwrap_or("");
                if !title.is_empty() {
                    let count = kv_read("rb_count").map(|s| parse_u32(s)).unwrap_or(0);
                    // Store metadata: title|category|servings|prep_time|rating
                    let mut w = W::new();
                    w.s(title); w.s("|"); w.s(category); w.s("|"); w.s(servings); w.s("|"); w.s(prep_time); w.s("|0");
                    let mut kb = [0u8; 16]; let kl = write_key(&mut kb, b"rb_", count);
                    kv_write(unsafe { core::str::from_utf8_unchecked(&kb[..kl]) }, w.out());
                    // Store ingredients separately
                    let mut ik = [0u8; 20]; let il = write_key(&mut ik, b"rb_i_", count);
                    kv_write(unsafe { core::str::from_utf8_unchecked(&ik[..il]) }, ingredients);
                    // Store instructions
                    let mut sk = [0u8; 20]; let sl = write_key(&mut sk, b"rb_s_", count);
                    kv_write(unsafe { core::str::from_utf8_unchecked(&sk[..sl]) }, instructions);
                    let mut cw = W::new(); cw.n(count + 1); kv_write("rb_count", cw.out());
                    respond(200, r#"{"ok":true}"#, "application/json");
                } else { respond(400, r#"{"error":"title required"}"#, "application/json"); }
            } else if action == "rate" {
                let idx = find_json_str(body, "index").map(|s| parse_u32(s)).unwrap_or(0);
                let rating = find_json_str(body, "rating").unwrap_or("0");
                let mut kb = [0u8; 16]; let kl = write_key(&mut kb, b"rb_", idx);
                let key = unsafe { core::str::from_utf8_unchecked(&kb[..kl]) };
                if let Some(data) = kv_read(key) {
                    // Replace last field (rating)
                    let db = data.as_bytes();
                    let mut last_pipe = 0; let mut pi = 0;
                    while pi < db.len() { if db[pi] == b'|' { last_pipe = pi; } pi += 1; }
                    let mut w = W::new();
                    w.s(&data[..last_pipe + 1]); w.s(rating);
                    kv_write(key, w.out());
                }
                respond(200, r#"{"ok":true}"#, "application/json");
            } else if action == "delete" {
                let idx = find_json_str(body, "index").map(|s| parse_u32(s)).unwrap_or(0);
                let mut kb = [0u8; 16]; let kl = write_key(&mut kb, b"rb_", idx);
                kv_write(unsafe { core::str::from_utf8_unchecked(&kb[..kl]) }, "");
                respond(200, r#"{"ok":true}"#, "application/json");
            } else { respond(400, r#"{"error":"unknown"}"#, "application/json"); }
        } else { respond(400, r#"{"error":"missing action"}"#, "application/json"); }
        return;
    }

    // GET — check if viewing a specific recipe
    let count = kv_read("rb_count").map(|s| parse_u32(s)).unwrap_or(0);

    // Check for /view/N path
    let pb = path.as_bytes();
    let viewing = if pb.len() > 6 && pb[0] == b'/' && pb[1] == b'v' && pb[2] == b'i' && pb[3] == b'e' && pb[4] == b'w' && pb[5] == b'/' {
        let num_s = unsafe { core::str::from_utf8_unchecked(&pb[6..]) };
        Some(parse_u32(num_s))
    } else { None };

    let mut w = W::new();
    w.s("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Recipe Book</title><style>");
    w.s("*{margin:0;padding:0;box-sizing:border-box}body{background:#1a0f0a;color:#e8ddd3;font-family:Georgia,serif;padding:30px 20px;display:flex;justify-content:center}");
    w.s(".c{max-width:700px;width:100%}h1{text-align:center;color:#e8a87c;margin-bottom:24px;font-size:2em}");
    w.s(".add-btn{display:block;width:100%;padding:14px;background:#2d1810;border:2px dashed #e8a87c;color:#e8a87c;border-radius:10px;cursor:pointer;font-size:16px;margin-bottom:20px;text-align:center;font-family:inherit}");
    w.s(".form{display:none;background:#2d1810;padding:20px;border-radius:12px;margin-bottom:20px}");
    w.s(".form.show{display:block}");
    w.s(".form input,.form textarea,.form select{width:100%;padding:10px;background:#1a0f0a;border:1px solid #4a3020;color:#e8ddd3;border-radius:6px;font-size:14px;margin-bottom:10px;font-family:inherit}");
    w.s(".form textarea{height:100px;resize:vertical}");
    w.s(".form .row{display:flex;gap:8px}.form .row>*{flex:1}");
    w.s("button{padding:10px 20px;background:#e8a87c;color:#1a0f0a;border:none;border-radius:6px;cursor:pointer;font-size:14px;font-weight:bold}");
    w.s(".recipe-card{background:#2d1810;padding:16px;border-radius:12px;margin-bottom:10px;cursor:pointer;transition:transform 0.2s}");
    w.s(".recipe-card:hover{transform:translateY(-2px)}");
    w.s(".recipe-card .title{font-size:18px;color:#e8a87c;font-weight:bold;margin-bottom:4px}");
    w.s(".recipe-card .meta{font-size:13px;color:#8a7a6a;display:flex;gap:16px}");
    w.s(".cat-badge{padding:2px 8px;border-radius:10px;font-size:11px;background:#3a2515;color:#e8a87c}");
    w.s(".stars{color:#e8a87c;font-size:16px}");
    w.s(".detail{background:#2d1810;padding:24px;border-radius:12px}");
    w.s(".detail h2{color:#e8a87c;margin-bottom:12px}.detail .meta{color:#8a7a6a;margin-bottom:16px;font-size:14px}");
    w.s(".detail h3{color:#c88a5c;margin:16px 0 8px;font-size:16px}");
    w.s(".detail .ingredients{list-style:none}.detail .ingredients li{padding:4px 0;border-bottom:1px solid #2a1a10;font-size:14px}");
    w.s(".detail .instructions{line-height:1.8;font-size:14px}");
    w.s(".back{margin-bottom:16px;background:transparent;border:1px solid #e8a87c;color:#e8a87c}");
    w.s(".rating{display:flex;gap:4px;margin-top:12px}.rating button{background:transparent;border:none;font-size:24px;cursor:pointer;padding:0}");
    w.s("</style></head><body><div class='c'><h1>Recipe Book</h1>");

    if let Some(idx) = viewing {
        // Detail view
        let mut kb = [0u8; 16]; let kl = write_key(&mut kb, b"rb_", idx);
        let key = unsafe { core::str::from_utf8_unchecked(&kb[..kl]) };
        if let Some(data) = kv_read(key) {
            if !data.is_empty() {
                let db = data.as_bytes();
                let mut pipes = [0usize; 4]; let mut pi = 0; let mut di = 0;
                while di < db.len() && pi < 4 { if db[di] == b'|' { pipes[pi] = di; pi += 1; } di += 1; }
                let title = &data[..pipes[0]];
                let category = &data[pipes[0]+1..pipes[1]];
                let servings = &data[pipes[1]+1..pipes[2]];
                let prep = &data[pipes[2]+1..pipes[3]];
                let rating = parse_u32(&data[pipes[3]+1..]);

                let mut ik = [0u8; 20]; let il = write_key(&mut ik, b"rb_i_", idx);
                let ingredients = kv_read(unsafe { core::str::from_utf8_unchecked(&ik[..il]) }).unwrap_or("");
                let mut sk = [0u8; 20]; let sl = write_key(&mut sk, b"rb_s_", idx);
                let instructions = kv_read(unsafe { core::str::from_utf8_unchecked(&sk[..sl]) }).unwrap_or("");

                w.s("<button class='back' onclick='goHome()'>Back to Recipes</button>");
                w.s("<div class='detail'><h2>"); w.s(title);
                w.s("</h2><div class='meta'><span class='cat-badge'>"); w.s(category);
                w.s("</span> | "); w.s(servings); w.s(" servings | "); w.s(prep); w.s(" min</div>");
                w.s("<div class='rating'>");
                let mut ri: u32 = 1;
                while ri <= 5 {
                    w.s("<button onclick=\"rate("); w.n(idx); w.s(","); w.n(ri); w.s(")\">");
                    if ri <= rating { w.s("&#9733;"); } else { w.s("&#9734;"); }
                    w.s("</button>");
                    ri += 1;
                }
                w.s("</div>");
                w.s("<h3>Ingredients</h3><ul class='ingredients'>");
                let ib = ingredients.as_bytes();
                let mut ip = 0;
                while ip < ib.len() {
                    let ls = ip;
                    while ip < ib.len() && ib[ip] != b',' { ip += 1; }
                    let item = unsafe { core::str::from_utf8_unchecked(&ib[ls..ip]) };
                    ip += 1;
                    if !item.is_empty() { w.s("<li>"); w.s(item); w.s("</li>"); }
                }
                w.s("</ul><h3>Instructions</h3><div class='instructions'>"); w.s(instructions);
                w.s("</div></div>");
            }
        }
    } else {
        // List view
        w.s("<div class='add-btn' onclick='toggleForm()'>+ Add New Recipe</div>");
        w.s("<div class='form' id='form'>");
        w.s("<input id='title' placeholder='Recipe name'>");
        w.s("<div class='row'><select id='cat'><option>breakfast</option><option>lunch</option><option>dinner</option><option>dessert</option><option>snack</option><option>drink</option></select>");
        w.s("<input type='number' id='servings' value='4' placeholder='Servings'><input type='number' id='prep' value='30' placeholder='Prep time (min)'></div>");
        w.s("<textarea id='ingredients' placeholder='Ingredients (comma-separated)'></textarea>");
        w.s("<textarea id='instructions' placeholder='Instructions'></textarea>");
        w.s("<button onclick='addRecipe()'>Save Recipe</button></div>");

        let mut i: u32 = 0;
        while i < count {
            let mut kb = [0u8; 16]; let kl = write_key(&mut kb, b"rb_", i);
            let key = unsafe { core::str::from_utf8_unchecked(&kb[..kl]) };
            if let Some(data) = kv_read(key) {
                if !data.is_empty() {
                    let db = data.as_bytes();
                    let mut pipes = [0usize; 4]; let mut pi = 0; let mut di = 0;
                    while di < db.len() && pi < 4 { if db[di] == b'|' { pipes[pi] = di; pi += 1; } di += 1; }
                    if pi >= 4 {
                        let title = &data[..pipes[0]];
                        let category = &data[pipes[0]+1..pipes[1]];
                        let servings = &data[pipes[1]+1..pipes[2]];
                        let prep = &data[pipes[2]+1..pipes[3]];
                        let rating = parse_u32(&data[pipes[3]+1..]);
                        w.s("<div class='recipe-card' onclick='view("); w.n(i); w.s(")'>");
                        w.s("<div class='title'>"); w.s(title); w.s("</div>");
                        w.s("<div class='meta'><span class='cat-badge'>"); w.s(category);
                        w.s("</span><span>"); w.s(servings); w.s(" servings</span><span>");
                        w.s(prep); w.s(" min</span><span class='stars'>");
                        let mut ri: u32 = 0;
                        while ri < rating && ri < 5 { w.s("&#9733;"); ri += 1; }
                        w.s("</span></div></div>");
                    }
                }
            }
            i += 1;
        }
    }

    w.s("</div><script>const B=location.pathname;");
    w.s("function toggleForm(){document.getElementById('form').classList.toggle('show');}");
    w.s("function view(i){window.location=B+'/view/'+i;}");
    w.s("function goHome(){window.location=B;}");
    w.s("async function addRecipe(){const t=document.getElementById('title').value.trim();if(!t)return;const c=document.getElementById('cat').value;const s=document.getElementById('servings').value;const p=document.getElementById('prep').value;const ing=document.getElementById('ingredients').value;const inst=document.getElementById('instructions').value;await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'add_recipe',title:t,category:c,servings:s,prep_time:p,ingredients:ing,instructions:inst})});location.reload();}");
    w.s("async function rate(i,r){await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'rate',index:String(i),rating:String(r)})});location.reload();}");
    w.s("</script></body></html>");
    respond(200, w.out(), "text/html");
}
