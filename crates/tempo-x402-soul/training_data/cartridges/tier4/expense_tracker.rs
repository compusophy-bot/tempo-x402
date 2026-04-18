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

fn find_json_str<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let key_bytes = key.as_bytes();
    let json_bytes = json.as_bytes();
    let mut i = 0;
    while i + key_bytes.len() + 3 < json_bytes.len() {
        if json_bytes[i] == b'"' {
            let start = i + 1;
            if start + key_bytes.len() < json_bytes.len()
                && &json_bytes[start..start + key_bytes.len()] == key_bytes
                && json_bytes[start + key_bytes.len()] == b'"'
            {
                let mut j = start + key_bytes.len() + 1;
                while j < json_bytes.len() && (json_bytes[j] == b':' || json_bytes[j] == b' ') { j += 1; }
                if j < json_bytes.len() && json_bytes[j] == b'"' {
                    let val_start = j + 1;
                    let mut val_end = val_start;
                    while val_end < json_bytes.len() && json_bytes[val_end] != b'"' { val_end += 1; }
                    return core::str::from_utf8(&json_bytes[val_start..val_end]).ok();
                }
            }
        }
        i += 1;
    }
    None
}

fn kv_read(key: &str) -> Option<&'static str> {
    unsafe {
        let result = kv_get(key.as_ptr(), key.len() as i32);
        if result < 0 { return None; }
        let ptr = (result >> 32) as *const u8;
        let len = (result & 0xFFFFFFFF) as usize;
        let bytes = core::slice::from_raw_parts(ptr, len);
        core::str::from_utf8(bytes).ok()
    }
}

fn kv_write(key: &str, value: &str) {
    unsafe {
        kv_set(key.as_ptr(), key.len() as i32, value.as_ptr(), value.len() as i32);
    }
}

static mut BUF: [u8; 65536] = [0u8; 65536];

struct BufWriter {
    pos: usize,
}

impl BufWriter {
    fn new() -> Self { Self { pos: 0 } }

    fn push_str(&mut self, s: &str) {
        let bytes = s.as_bytes();
        unsafe {
            let end = (self.pos + bytes.len()).min(BUF.len());
            let copy_len = end - self.pos;
            BUF[self.pos..end].copy_from_slice(&bytes[..copy_len]);
            self.pos = end;
        }
    }

    fn push_num(&mut self, mut n: u32) {
        if n == 0 { self.push_str("0"); return; }
        let mut digits = [0u8; 10];
        let mut i = 0;
        while n > 0 { digits[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; }
        while i > 0 { i -= 1; unsafe { if self.pos < BUF.len() { BUF[self.pos] = digits[i]; self.pos += 1; } } }
    }

    fn as_str(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&BUF[..self.pos]) }
    }
}

fn parse_num(s: &str) -> u32 {
    let mut n: u32 = 0;
    for &b in s.as_bytes() {
        if b >= b'0' && b <= b'9' { n = n * 10 + (b - b'0') as u32; }
    }
    n
}

fn make_key<'a>(buf: &'a mut [u8; 32], prefix: &str, num: u32) -> &'a str {
    let pb = prefix.as_bytes();
    let mut pos = 0;
    while pos < pb.len() && pos < 32 { buf[pos] = pb[pos]; pos += 1; }
    let mut n = num;
    if n == 0 { buf[pos] = b'0'; pos += 1; }
    else { let start = pos; while n > 0 { buf[pos] = b'0' + (n % 10) as u8; n /= 10; pos += 1; } buf[start..pos].reverse(); }
    unsafe { core::str::from_utf8_unchecked(&buf[..pos]) }
}

fn num_to_str<'a>(buf: &'a mut [u8; 10], mut n: u32) -> &'a str {
    if n == 0 { buf[0] = b'0'; return unsafe { core::str::from_utf8_unchecked(&buf[..1]) }; }
    let mut pos = 0;
    while n > 0 { buf[pos] = b'0' + (n % 10) as u8; n /= 10; pos += 1; }
    buf[..pos].reverse();
    unsafe { core::str::from_utf8_unchecked(&buf[..pos]) }
}

fn bytes_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    let mut i = 0;
    while i < a.len() { if a[i] != b[i] { return false; } i += 1; }
    true
}

#[no_mangle]
pub extern "C" fn handle(method_ptr: *const u8, method_len: i32, _path_ptr: *const u8, _path_len: i32, body_ptr: *const u8, body_len: i32) {
    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };
    let body = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(body_ptr, body_len as usize)) };

    if method.as_bytes()[0] == b'P' {
        if let Some(action) = find_json_str(body, "action") {
            if bytes_eq(action.as_bytes(), b"add") {
                let desc = find_json_str(body, "desc").unwrap_or("Unknown");
                let amount = find_json_str(body, "amount").unwrap_or("0");
                let category = find_json_str(body, "category").unwrap_or("other");

                let count = parse_num(kv_read("exp_count").unwrap_or("0"));
                let new_id = count + 1;

                let mut w = BufWriter::new();
                w.push_str("{\"desc\":\"");
                w.push_str(desc);
                w.push_str("\",\"amount\":\"");
                w.push_str(amount);
                w.push_str("\",\"category\":\"");
                w.push_str(category);
                w.push_str("\"}");

                let mut kb = [0u8; 32];
                kv_write(make_key(&mut kb, "exp_", new_id), w.as_str());
                let mut nb = [0u8; 10];
                kv_write("exp_count", num_to_str(&mut nb, new_id));

                respond(200, "{\"ok\":true}", "application/json");
            } else if bytes_eq(action.as_bytes(), b"delete") {
                if let Some(id_str) = find_json_str(body, "id") {
                    let id = parse_num(id_str);
                    let mut kb = [0u8; 32];
                    kv_write(make_key(&mut kb, "exp_", id), "");
                    respond(200, "{\"ok\":true}", "application/json");
                } else {
                    respond(400, "{\"error\":\"missing id\"}", "application/json");
                }
            } else {
                respond(400, "{\"error\":\"unknown action\"}", "application/json");
            }
        } else {
            respond(400, "{\"error\":\"missing action\"}", "application/json");
        }
        return;
    }

    // GET: render expense tracker
    let count = parse_num(kv_read("exp_count").unwrap_or("0"));
    let mut w = BufWriter::new();
    w.push_str("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Expense Tracker</title><style>");
    w.push_str("*{margin:0;padding:0;box-sizing:border-box}");
    w.push_str("body{background:#0a0a0f;color:#e0e0e0;font-family:'Segoe UI',sans-serif;min-height:100vh;padding:40px 20px}");
    w.push_str(".container{max-width:700px;margin:0 auto}");
    w.push_str("h1{text-align:center;color:#f59e0b;margin-bottom:8px;font-size:2em}");
    w.push_str(".subtitle{text-align:center;color:#666;margin-bottom:32px}");
    w.push_str(".total-card{background:linear-gradient(135deg,#1a1a2e,#16213e);border:1px solid #f59e0b33;border-radius:16px;padding:24px;text-align:center;margin-bottom:24px}");
    w.push_str(".total-label{color:#f59e0b;font-size:14px;text-transform:uppercase;letter-spacing:2px}");
    w.push_str(".total-amount{font-size:48px;font-weight:bold;color:#f59e0b;margin:8px 0}");
    w.push_str(".categories{display:grid;grid-template-columns:repeat(auto-fit,minmax(120px,1fr));gap:12px;margin-bottom:24px}");
    w.push_str(".cat-card{background:#1a1a2e;border-radius:10px;padding:14px;text-align:center}");
    w.push_str(".cat-name{font-size:12px;color:#888;text-transform:uppercase;letter-spacing:1px}");
    w.push_str(".cat-amount{font-size:20px;font-weight:bold;margin-top:4px}");
    w.push_str(".add-form{background:#1a1a2e;border-radius:12px;padding:20px;margin-bottom:24px;display:grid;grid-template-columns:2fr 1fr 1fr auto;gap:10px;align-items:end}");
    w.push_str(".field label{display:block;font-size:12px;color:#888;margin-bottom:4px;text-transform:uppercase}");
    w.push_str(".field input,.field select{width:100%;padding:10px;border:1px solid #333;background:#0a0a0f;color:#e0e0e0;border-radius:6px;font-size:14px;outline:none}");
    w.push_str(".add-btn{padding:10px 20px;background:#f59e0b;color:#000;border:none;border-radius:6px;cursor:pointer;font-weight:bold;font-size:14px;height:38px}");
    w.push_str(".expense-list{background:#1a1a2e;border-radius:12px;overflow:hidden}");
    w.push_str(".exp-item{display:flex;align-items:center;padding:14px 20px;border-bottom:1px solid #222}");
    w.push_str(".exp-item:last-child{border-bottom:none}");
    w.push_str(".exp-cat{padding:4px 10px;border-radius:20px;font-size:11px;font-weight:bold;text-transform:uppercase;margin-right:14px}");
    w.push_str(".cat-food{background:#22c55e22;color:#22c55e}.cat-transport{background:#3b82f622;color:#3b82f6}");
    w.push_str(".cat-entertainment{background:#a855f722;color:#a855f7}.cat-bills{background:#ef444422;color:#ef4444}");
    w.push_str(".cat-shopping{background:#ec489922;color:#ec4899}.cat-other{background:#6b728022;color:#9ca3af}");
    w.push_str(".exp-desc{flex:1;font-size:15px}");
    w.push_str(".exp-amount{font-size:16px;font-weight:bold;color:#f59e0b;margin-right:14px}");
    w.push_str(".exp-del{padding:4px 10px;background:transparent;border:1px solid #333;color:#888;border-radius:4px;cursor:pointer;font-size:12px}");
    w.push_str(".exp-del:hover{border-color:#ef4444;color:#ef4444}");
    w.push_str("</style></head><body><div class='container'>");
    w.push_str("<h1>Expense Tracker</h1><p class='subtitle'>Track your spending</p>");

    // Calculate totals per category
    let categories = ["food", "transport", "entertainment", "bills", "shopping", "other"];
    let mut cat_totals = [0u32; 6];
    let mut grand_total: u32 = 0;

    let mut i: u32 = 1;
    while i <= count {
        let mut kb = [0u8; 32];
        if let Some(data) = kv_read(make_key(&mut kb, "exp_", i)) {
            if data.len() > 0 {
                let amount = parse_num(find_json_str(data, "amount").unwrap_or("0"));
                let cat = find_json_str(data, "category").unwrap_or("other");
                grand_total += amount;
                let mut ci = 0;
                while ci < 6 {
                    if bytes_eq(cat.as_bytes(), categories[ci].as_bytes()) {
                        cat_totals[ci] += amount;
                        break;
                    }
                    ci += 1;
                }
                if ci == 6 { cat_totals[5] += amount; }
            }
        }
        i += 1;
    }

    // Total card
    w.push_str("<div class='total-card'><div class='total-label'>Total Spent</div><div class='total-amount'>$");
    w.push_num(grand_total / 100);
    w.push_str(".");
    let cents = grand_total % 100;
    if cents < 10 { w.push_str("0"); }
    w.push_num(cents);
    w.push_str("</div></div>");

    // Category breakdown
    w.push_str("<div class='categories'>");
    let cat_colors = ["#22c55e", "#3b82f6", "#a855f7", "#ef4444", "#ec4899", "#9ca3af"];
    let mut ci = 0;
    while ci < 6 {
        w.push_str("<div class='cat-card'><div class='cat-name'>");
        w.push_str(categories[ci]);
        w.push_str("</div><div class='cat-amount' style='color:");
        w.push_str(cat_colors[ci]);
        w.push_str("'>$");
        w.push_num(cat_totals[ci] / 100);
        w.push_str(".");
        let c = cat_totals[ci] % 100;
        if c < 10 { w.push_str("0"); }
        w.push_num(c);
        w.push_str("</div></div>");
        ci += 1;
    }
    w.push_str("</div>");

    // Add form
    w.push_str("<div class='add-form'>");
    w.push_str("<div class='field'><label>Description</label><input type='text' id='desc' placeholder='What did you buy?'></div>");
    w.push_str("<div class='field'><label>Amount ($)</label><input type='number' id='amount' step='0.01' placeholder='0.00'></div>");
    w.push_str("<div class='field'><label>Category</label><select id='category'>");
    w.push_str("<option value='food'>Food</option><option value='transport'>Transport</option>");
    w.push_str("<option value='entertainment'>Entertainment</option><option value='bills'>Bills</option>");
    w.push_str("<option value='shopping'>Shopping</option><option value='other'>Other</option>");
    w.push_str("</select></div>");
    w.push_str("<button class='add-btn' onclick='addExpense()'>Add</button></div>");

    // Expense list
    w.push_str("<div class='expense-list'>");
    let mut j = count;
    while j >= 1 {
        let mut kb = [0u8; 32];
        if let Some(data) = kv_read(make_key(&mut kb, "exp_", j)) {
            if data.len() > 0 {
                let desc = find_json_str(data, "desc").unwrap_or("?");
                let amount = parse_num(find_json_str(data, "amount").unwrap_or("0"));
                let cat = find_json_str(data, "category").unwrap_or("other");
                w.push_str("<div class='exp-item'><span class='exp-cat cat-");
                w.push_str(cat);
                w.push_str("'>");
                w.push_str(cat);
                w.push_str("</span><span class='exp-desc'>");
                w.push_str(desc);
                w.push_str("</span><span class='exp-amount'>$");
                w.push_num(amount / 100);
                w.push_str(".");
                let c = amount % 100;
                if c < 10 { w.push_str("0"); }
                w.push_num(c);
                w.push_str("</span><button class='exp-del' onclick='deleteExpense(");
                w.push_num(j);
                w.push_str(")'>X</button></div>");
            }
        }
        j -= 1;
    }
    w.push_str("</div></div>");

    w.push_str("<script>");
    w.push_str("const BASE=location.pathname;");
    w.push_str("async function addExpense(){const desc=document.getElementById('desc').value.trim();const amountStr=document.getElementById('amount').value;const category=document.getElementById('category').value;if(!desc||!amountStr)return;const cents=Math.round(parseFloat(amountStr)*100);await fetch(BASE,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'add',desc,amount:String(cents),category})});location.reload();}");
    w.push_str("async function deleteExpense(id){await fetch(BASE,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'delete',id:String(id)})});location.reload();}");
    w.push_str("document.getElementById('amount').addEventListener('keydown',function(e){if(e.key==='Enter')addExpense();});");
    w.push_str("</script></body></html>");

    respond(200, w.as_str(), "text/html");
}
