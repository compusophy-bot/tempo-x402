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

fn find_json_str<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let kb = key.as_bytes();
    let jb = json.as_bytes();
    let mut i = 0;
    while i + kb.len() + 3 < jb.len() {
        if jb[i] == b'"' {
            let s = i + 1;
            if s + kb.len() < jb.len() && &jb[s..s + kb.len()] == kb && jb[s + kb.len()] == b'"' {
                let mut j = s + kb.len() + 1;
                while j < jb.len() && (jb[j] == b':' || jb[j] == b' ') { j += 1; }
                if j < jb.len() && jb[j] == b'"' {
                    let vs = j + 1;
                    let mut ve = vs;
                    while ve < jb.len() && jb[ve] != b'"' { ve += 1; }
                    return core::str::from_utf8(&jb[vs..ve]).ok();
                }
            }
        }
        i += 1;
    }
    None
}

fn kv_read(key: &str) -> Option<&'static str> {
    unsafe {
        let r = kv_get(key.as_ptr(), key.len() as i32);
        if r < 0 { return None; }
        let ptr = (r >> 32) as *const u8;
        let len = (r & 0xFFFFFFFF) as usize;
        core::str::from_utf8(core::slice::from_raw_parts(ptr, len)).ok()
    }
}

fn kv_write(key: &str, value: &str) {
    unsafe { kv_set(key.as_ptr(), key.len() as i32, value.as_ptr(), value.len() as i32); }
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}

static mut BUF: [u8; 65536] = [0u8; 65536];
fn buf_write(pos: usize, s: &str) -> usize {
    let b = s.as_bytes();
    let end = (pos + b.len()).min(unsafe { BUF.len() });
    unsafe { BUF[pos..end].copy_from_slice(&b[..end - pos]); }
    end
}
fn buf_as_str(len: usize) -> &'static str {
    unsafe { core::str::from_utf8_unchecked(&BUF[..len]) }
}

fn write_usize(pos: usize, mut n: usize) -> usize {
    if n == 0 { return buf_write(pos, "0"); }
    static mut DIGITS: [u8; 20] = [0u8; 20];
    let mut i = 0;
    while n > 0 { unsafe { DIGITS[i] = b'0' + (n % 10) as u8; } n /= 10; i += 1; }
    let mut p = pos;
    while i > 0 { i -= 1; let d = unsafe { DIGITS[i] }; let s = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(&d, 1)) }; p = buf_write(p, s); }
    p
}

fn parse_usize(s: &str) -> usize {
    let mut n = 0usize;
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() { if b[i] >= b'0' && b[i] <= b'9' { n = n * 10 + (b[i] - b'0') as usize; } i += 1; }
    n
}

#[no_mangle]
pub extern "C" fn handle_request(method_ptr: *const u8, method_len: i32, path_ptr: *const u8, path_len: i32, body_ptr: *const u8, body_len: i32) {
    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };
    let body = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(body_ptr, body_len as usize)) };

    if method == "POST" {
        let action = find_json_str(body, "action").unwrap_or("add");
        let name = find_json_str(body, "name").unwrap_or("");
        let qty = find_json_str(body, "qty").unwrap_or("0");
        let threshold = find_json_str(body, "threshold").unwrap_or("5");
        let idx_str = find_json_str(body, "index").unwrap_or("0");

        if action == "add" && name.len() > 0 {
            let existing = kv_read("inventory").unwrap_or("");
            let mut p = 0usize;
            p = buf_write(p, existing);
            p = buf_write(p, name);
            p = buf_write(p, "|");
            p = buf_write(p, qty);
            p = buf_write(p, "|");
            p = buf_write(p, threshold);
            p = buf_write(p, "\n");
            kv_write("inventory", buf_as_str(p));
        } else if action == "delete" {
            let existing = kv_read("inventory").unwrap_or("");
            let target = parse_usize(idx_str);
            let eb = existing.as_bytes();
            let mut epos = 0;
            let mut count = 0usize;
            static mut NEW_INV: [u8; 16384] = [0u8; 16384];
            let mut np = 0usize;
            while epos < eb.len() {
                let mut eend = epos;
                while eend < eb.len() && eb[eend] != b'\n' { eend += 1; }
                if eend > epos && count != target {
                    let line = &eb[epos..eend];
                    unsafe { NEW_INV[np..np+line.len()].copy_from_slice(line); np += line.len(); NEW_INV[np] = b'\n'; np += 1; }
                }
                if eend > epos { count += 1; }
                epos = eend + 1;
            }
            kv_write("inventory", unsafe { core::str::from_utf8_unchecked(&NEW_INV[..np]) });
        } else if action == "update" {
            let existing = kv_read("inventory").unwrap_or("");
            let target = parse_usize(idx_str);
            let new_qty = qty;
            let eb = existing.as_bytes();
            let mut epos = 0;
            let mut count = 0usize;
            static mut UPD_INV: [u8; 16384] = [0u8; 16384];
            let mut np = 0usize;
            while epos < eb.len() {
                let mut eend = epos;
                while eend < eb.len() && eb[eend] != b'\n' { eend += 1; }
                if eend > epos {
                    if count == target {
                        let line = &eb[epos..eend];
                        // Parse existing line to get name and threshold
                        let mut seps: [usize; 2] = [0; 2];
                        let mut sc = 0;
                        let mut si = 0;
                        while si < line.len() && sc < 2 { if line[si] == b'|' { seps[sc] = si; sc += 1; } si += 1; }
                        if sc >= 2 {
                            let iname = &line[..seps[0]];
                            let ithresh = &line[seps[1]+1..];
                            unsafe {
                                UPD_INV[np..np+iname.len()].copy_from_slice(iname); np += iname.len();
                                UPD_INV[np] = b'|'; np += 1;
                            }
                            let qb = new_qty.as_bytes();
                            unsafe { UPD_INV[np..np+qb.len()].copy_from_slice(qb); np += qb.len(); UPD_INV[np] = b'|'; np += 1; }
                            unsafe { UPD_INV[np..np+ithresh.len()].copy_from_slice(ithresh); np += ithresh.len(); UPD_INV[np] = b'\n'; np += 1; }
                        }
                    } else {
                        let line = &eb[epos..eend];
                        unsafe { UPD_INV[np..np+line.len()].copy_from_slice(line); np += line.len(); UPD_INV[np] = b'\n'; np += 1; }
                    }
                    count += 1;
                }
                epos = eend + 1;
            }
            kv_write("inventory", unsafe { core::str::from_utf8_unchecked(&UPD_INV[..np]) });
        }
    }

    let inventory = kv_read("inventory").unwrap_or("");

    let mut p = 0usize;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Inventory Tracker</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{background:#121212;color:#e0e0e0;font-family:'Segoe UI',sans-serif;min-height:100vh;padding:20px;display:flex;flex-direction:column;align-items:center}
h1{color:#bb86fc;margin:20px 0;font-size:2em}
.container{width:100%;max-width:650px}
.add-form{background:#1e1e1e;border-radius:12px;padding:20px;margin-bottom:20px;border:1px solid #333}
.add-form h2{color:#bb86fc;margin-bottom:12px}
.form-row{display:flex;gap:10px;margin-bottom:10px;flex-wrap:wrap}
.form-row input{flex:1;min-width:80px;padding:10px;border:1px solid #333;border-radius:8px;background:#121212;color:#e0e0e0;font-size:1em}
.form-row input:focus{outline:none;border-color:#bb86fc}
.add-btn{width:100%;padding:12px;background:#bb86fc;color:#121212;border:none;border-radius:8px;font-size:1em;cursor:pointer;font-weight:bold}
.add-btn:hover{background:#9c64f0}
.stats{display:flex;gap:15px;margin-bottom:20px}
.stat{flex:1;background:#1e1e1e;border:1px solid #333;border-radius:12px;padding:15px;text-align:center}
.stat .num{font-size:1.8em;font-weight:bold;color:#bb86fc}
.stat .label{color:#888;font-size:0.85em;margin-top:4px}
.stat.warn .num{color:#cf6679}
.item{background:#1e1e1e;border:1px solid #333;border-radius:10px;padding:15px;margin-bottom:8px;display:flex;align-items:center;gap:15px}
.item.low-stock{border-color:#cf6679;background:#1e1a1a}
.item .info{flex:1}
.item .name{font-weight:bold;font-size:1.05em;color:#e0e0e0}
.item .meta{color:#888;font-size:0.85em;margin-top:2px}
.item .qty{font-size:1.4em;font-weight:bold;color:#03dac6;min-width:50px;text-align:center}
.item.low-stock .qty{color:#cf6679}
.item .warning{color:#cf6679;font-size:0.8em;font-weight:bold}
.item-actions{display:flex;gap:6px}
.item-actions button{padding:6px 10px;border:none;border-radius:6px;cursor:pointer;font-size:0.85em}
.btn-inc{background:#03dac6;color:#121212}
.btn-dec{background:#cf6679;color:#fff}
.btn-del{background:#333;color:#e0e0e0}
.empty{text-align:center;color:#666;padding:40px;font-size:1.1em}
</style></head><body>
<h1>&#128230; Inventory Tracker</h1>
<div class="container">
"##);

    // Parse and count
    let ib = inventory.as_bytes();
    let mut ipos = 0;
    let mut total_items = 0usize;
    let mut low_count = 0usize;
    let mut total_qty = 0usize;

    // First pass: stats
    while ipos < ib.len() {
        let mut iend = ipos;
        while iend < ib.len() && ib[iend] != b'\n' { iend += 1; }
        if iend > ipos {
            let line = &ib[ipos..iend];
            let mut seps: [usize; 2] = [0; 2];
            let mut sc = 0;
            let mut si = 0;
            while si < line.len() && sc < 2 { if line[si] == b'|' { seps[sc] = si; sc += 1; } si += 1; }
            if sc >= 2 {
                let q = parse_usize(unsafe { core::str::from_utf8_unchecked(&line[seps[0]+1..seps[1]]) });
                let t = parse_usize(unsafe { core::str::from_utf8_unchecked(&line[seps[1]+1..]) });
                total_items += 1;
                total_qty += q;
                if q <= t { low_count += 1; }
            }
        }
        ipos = iend + 1;
    }

    p = buf_write(p, r##"<div class="stats"><div class="stat"><div class="num">"##);
    p = write_usize(p, total_items);
    p = buf_write(p, r##"</div><div class="label">Items</div></div><div class="stat"><div class="num">"##);
    p = write_usize(p, total_qty);
    p = buf_write(p, r##"</div><div class="label">Total Qty</div></div><div class="stat warn"><div class="num">"##);
    p = write_usize(p, low_count);
    p = buf_write(p, r##"</div><div class="label">Low Stock</div></div></div>"##);

    p = buf_write(p, r##"<div class="add-form"><h2>Add Item</h2>
<div class="form-row">
<input type="text" id="name" placeholder="Item name">
<input type="number" id="qty" placeholder="Qty" min="0" value="1">
<input type="number" id="threshold" placeholder="Low stock at" min="0" value="5">
</div>
<button class="add-btn" onclick="addItem()">Add Item</button></div>"##);

    // Render items
    ipos = 0;
    let mut idx = 0usize;
    if total_items == 0 {
        p = buf_write(p, r##"<div class="empty">No items in inventory. Add your first item above!</div>"##);
    }
    while ipos < ib.len() {
        let mut iend = ipos;
        while iend < ib.len() && ib[iend] != b'\n' { iend += 1; }
        if iend > ipos {
            let line = &ib[ipos..iend];
            let mut seps: [usize; 2] = [0; 2];
            let mut sc = 0;
            let mut si = 0;
            while si < line.len() && sc < 2 { if line[si] == b'|' { seps[sc] = si; sc += 1; } si += 1; }
            if sc >= 2 {
                let iname = unsafe { core::str::from_utf8_unchecked(&line[..seps[0]]) };
                let qty = parse_usize(unsafe { core::str::from_utf8_unchecked(&line[seps[0]+1..seps[1]]) });
                let thresh = parse_usize(unsafe { core::str::from_utf8_unchecked(&line[seps[1]+1..]) });
                let is_low = qty <= thresh;
                if is_low {
                    p = buf_write(p, r##"<div class="item low-stock">"##);
                } else {
                    p = buf_write(p, r##"<div class="item">"##);
                }
                p = buf_write(p, r##"<div class="info"><div class="name">"##);
                p = buf_write(p, iname);
                p = buf_write(p, r##"</div><div class="meta">Low stock threshold: "##);
                p = write_usize(p, thresh);
                p = buf_write(p, "</div>");
                if is_low {
                    p = buf_write(p, r##"<div class="warning">&#9888; LOW STOCK</div>"##);
                }
                p = buf_write(p, r##"</div><div class="qty">"##);
                p = write_usize(p, qty);
                p = buf_write(p, r##"</div><div class="item-actions"><button class="btn-inc" onclick="updateQty("##);
                p = write_usize(p, idx);
                p = buf_write(p, ",");
                p = write_usize(p, qty + 1);
                p = buf_write(p, r##")">+</button><button class="btn-dec" onclick="updateQty("##);
                p = write_usize(p, idx);
                p = buf_write(p, ",");
                if qty > 0 { p = write_usize(p, qty - 1); } else { p = buf_write(p, "0"); }
                p = buf_write(p, r##")">-</button><button class="btn-del" onclick="delItem("##);
                p = write_usize(p, idx);
                p = buf_write(p, r##")">&#128465;</button></div></div>"##);
                idx += 1;
            }
        }
        ipos = iend + 1;
    }

    p = buf_write(p, r##"</div>
<script>
function addItem(){var n=document.getElementById('name').value;var q=document.getElementById('qty').value;var t=document.getElementById('threshold').value||'5';if(!n)return alert('Enter item name');fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'add',name:n,qty:q,threshold:t})}).then(function(){location.reload()})}
function updateQty(i,q){fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'update',index:String(i),qty:String(q)})}).then(function(){location.reload()})}
function delItem(i){if(!confirm('Delete this item?'))return;fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'delete',index:String(i)})}).then(function(){location.reload()})}
</script></body></html>"##);

    respond(200, buf_as_str(p), "text/html");
}
