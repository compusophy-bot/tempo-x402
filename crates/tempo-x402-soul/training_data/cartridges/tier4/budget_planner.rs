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
        let category = find_json_str(body, "category").unwrap_or("");
        let budget = find_json_str(body, "budget").unwrap_or("0");
        let spent = find_json_str(body, "spent").unwrap_or("0");

        if action == "add" && category.len() > 0 {
            let existing = kv_read("budget_cats").unwrap_or("");
            let mut bp = 0usize;
            bp = buf_write(bp, existing);
            bp = buf_write(bp, category);
            bp = buf_write(bp, "|");
            bp = buf_write(bp, budget);
            bp = buf_write(bp, "|");
            bp = buf_write(bp, spent);
            bp = buf_write(bp, "\n");
            kv_write("budget_cats", buf_as_str(bp));
        } else if action == "spend" {
            let idx_str = find_json_str(body, "index").unwrap_or("0");
            let amount = find_json_str(body, "amount").unwrap_or("0");
            let target = parse_usize(idx_str);
            let add_amount = parse_usize(amount);
            let existing = kv_read("budget_cats").unwrap_or("");
            let eb = existing.as_bytes();
            static mut UPD: [u8; 16384] = [0u8; 16384];
            let mut np = 0usize;
            let mut epos = 0;
            let mut count = 0usize;
            while epos < eb.len() {
                let mut eend = epos;
                while eend < eb.len() && eb[eend] != b'\n' { eend += 1; }
                if eend > epos {
                    let line = &eb[epos..eend];
                    if count == target {
                        let mut seps: [usize; 2] = [0; 2];
                        let mut sc = 0;
                        let mut si = 0;
                        while si < line.len() && sc < 2 { if line[si] == b'|' { seps[sc] = si; sc += 1; } si += 1; }
                        if sc >= 2 {
                            let cat = &line[..seps[0]];
                            let bud = &line[seps[0]+1..seps[1]];
                            let old_spent = parse_usize(unsafe { core::str::from_utf8_unchecked(&line[seps[1]+1..]) });
                            let new_spent = old_spent + add_amount;
                            unsafe { UPD[np..np+cat.len()].copy_from_slice(cat); np += cat.len(); UPD[np] = b'|'; np += 1; }
                            unsafe { UPD[np..np+bud.len()].copy_from_slice(bud); np += bud.len(); UPD[np] = b'|'; np += 1; }
                            // Write new spent
                            let mut tmp = [0u8; 20];
                            let mut ti = 0;
                            let mut ns = new_spent;
                            if ns == 0 { tmp[0] = b'0'; ti = 1; } else {
                                while ns > 0 { tmp[ti] = b'0' + (ns % 10) as u8; ns /= 10; ti += 1; }
                            }
                            let mut tj = ti;
                            while tj > 0 { tj -= 1; unsafe { UPD[np] = tmp[tj]; np += 1; } }
                            unsafe { UPD[np] = b'\n'; np += 1; }
                        }
                    } else {
                        unsafe { UPD[np..np+line.len()].copy_from_slice(line); np += line.len(); UPD[np] = b'\n'; np += 1; }
                    }
                    count += 1;
                }
                epos = eend + 1;
            }
            kv_write("budget_cats", unsafe { core::str::from_utf8_unchecked(&UPD[..np]) });
        }
    }

    let cats = kv_read("budget_cats").unwrap_or("");

    let mut p = 0usize;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Budget Planner</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{background:#0a0a0a;color:#e8e8e8;font-family:'Segoe UI',sans-serif;min-height:100vh;padding:20px;display:flex;flex-direction:column;align-items:center}
h1{color:#00d4aa;margin:20px 0;font-size:2em}
.container{width:100%;max-width:600px}
.summary{display:flex;gap:15px;margin-bottom:20px}
.summary-card{flex:1;background:#1a1a1a;border-radius:12px;padding:18px;text-align:center;border:1px solid #2a2a2a}
.summary-card .val{font-size:1.6em;font-weight:bold;margin-top:5px}
.summary-card .lbl{color:#888;font-size:0.85em}
.green{color:#00d4aa}
.red{color:#ff6b6b}
.yellow{color:#ffd93d}
.add-form{background:#1a1a1a;border-radius:12px;padding:20px;margin-bottom:20px;border:1px solid #2a2a2a}
.add-form h2{color:#00d4aa;margin-bottom:12px}
.form-row{display:flex;gap:10px}
.form-row input{flex:1;padding:10px;border:1px solid #2a2a2a;border-radius:8px;background:#0a0a0a;color:#e8e8e8;font-size:1em}
.form-row input:focus{outline:none;border-color:#00d4aa}
.add-btn{padding:10px 20px;background:#00d4aa;color:#0a0a0a;border:none;border-radius:8px;cursor:pointer;font-weight:bold}
.cat{background:#1a1a1a;border:1px solid #2a2a2a;border-radius:12px;padding:18px;margin-bottom:12px}
.cat-header{display:flex;justify-content:space-between;align-items:center;margin-bottom:10px}
.cat-name{font-size:1.1em;font-weight:bold}
.cat-nums{font-size:0.9em;color:#888}
.bar-bg{height:24px;background:#2a2a2a;border-radius:12px;overflow:hidden;position:relative}
.bar-fill{height:100%;border-radius:12px;transition:width 0.3s}
.bar-fill.ok{background:linear-gradient(90deg,#00d4aa,#00b894)}
.bar-fill.warn{background:linear-gradient(90deg,#ffd93d,#fdcb6e)}
.bar-fill.over{background:linear-gradient(90deg,#ff6b6b,#e74c3c)}
.bar-pct{position:absolute;right:8px;top:3px;font-size:0.75em;font-weight:bold;color:#fff}
.spend-row{display:flex;gap:8px;margin-top:10px}
.spend-row input{flex:1;padding:8px;border:1px solid #2a2a2a;border-radius:6px;background:#0a0a0a;color:#e8e8e8}
.spend-btn{padding:8px 16px;background:#ff6b6b;color:#fff;border:none;border-radius:6px;cursor:pointer;font-weight:bold;font-size:0.9em}
.empty{text-align:center;color:#555;padding:40px;font-size:1.1em}
</style></head><body>
<h1>&#128176; Budget Planner</h1>
<div class="container">
"##);

    // Compute totals
    let cb = cats.as_bytes();
    let mut cpos = 0;
    let mut total_budget = 0usize;
    let mut total_spent = 0usize;
    let mut cat_count = 0usize;

    while cpos < cb.len() {
        let mut cend = cpos;
        while cend < cb.len() && cb[cend] != b'\n' { cend += 1; }
        if cend > cpos {
            let line = &cb[cpos..cend];
            let mut seps: [usize; 2] = [0; 2];
            let mut sc = 0;
            let mut si = 0;
            while si < line.len() && sc < 2 { if line[si] == b'|' { seps[sc] = si; sc += 1; } si += 1; }
            if sc >= 2 {
                total_budget += parse_usize(unsafe { core::str::from_utf8_unchecked(&line[seps[0]+1..seps[1]]) });
                total_spent += parse_usize(unsafe { core::str::from_utf8_unchecked(&line[seps[1]+1..]) });
                cat_count += 1;
            }
        }
        cpos = cend + 1;
    }

    let remaining = if total_budget > total_spent { total_budget - total_spent } else { 0 };

    p = buf_write(p, r##"<div class="summary"><div class="summary-card"><div class="lbl">Total Budget</div><div class="val green">$"##);
    p = write_usize(p, total_budget);
    p = buf_write(p, r##"</div></div><div class="summary-card"><div class="lbl">Spent</div><div class="val red">$"##);
    p = write_usize(p, total_spent);
    p = buf_write(p, r##"</div></div><div class="summary-card"><div class="lbl">Remaining</div><div class="val yellow">$"##);
    p = write_usize(p, remaining);
    p = buf_write(p, r##"</div></div></div>"##);

    p = buf_write(p, r##"<div class="add-form"><h2>Add Category</h2><div class="form-row">
<input type="text" id="category" placeholder="Category name">
<input type="number" id="budget" placeholder="Budget ($)" min="0">
<button class="add-btn" onclick="addCat()">Add</button></div></div>"##);

    // Render categories with bars
    cpos = 0;
    let mut idx = 0usize;
    if cat_count == 0 {
        p = buf_write(p, r##"<div class="empty">No budget categories yet. Add one to start planning!</div>"##);
    }
    while cpos < cb.len() {
        let mut cend = cpos;
        while cend < cb.len() && cb[cend] != b'\n' { cend += 1; }
        if cend > cpos {
            let line = &cb[cpos..cend];
            let mut seps: [usize; 2] = [0; 2];
            let mut sc = 0;
            let mut si = 0;
            while si < line.len() && sc < 2 { if line[si] == b'|' { seps[sc] = si; sc += 1; } si += 1; }
            if sc >= 2 {
                let name = unsafe { core::str::from_utf8_unchecked(&line[..seps[0]]) };
                let bud = parse_usize(unsafe { core::str::from_utf8_unchecked(&line[seps[0]+1..seps[1]]) });
                let spt = parse_usize(unsafe { core::str::from_utf8_unchecked(&line[seps[1]+1..]) });
                let pct = if bud > 0 { (spt * 100) / bud } else { 100 };
                let bar_class = if pct > 100 { "over" } else if pct > 75 { "warn" } else { "ok" };
                let bar_width = if pct > 100 { 100 } else { pct };

                p = buf_write(p, r##"<div class="cat"><div class="cat-header"><span class="cat-name">"##);
                p = buf_write(p, name);
                p = buf_write(p, r##"</span><span class="cat-nums">$"##);
                p = write_usize(p, spt);
                p = buf_write(p, " / $");
                p = write_usize(p, bud);
                p = buf_write(p, r##"</span></div><div class="bar-bg"><div class="bar-fill "##);
                p = buf_write(p, bar_class);
                p = buf_write(p, r##"" style="width:"##);
                p = write_usize(p, bar_width);
                p = buf_write(p, r##"%"></div><span class="bar-pct">"##);
                p = write_usize(p, pct);
                p = buf_write(p, r##"%</span></div><div class="spend-row"><input type="number" id="spend-"##);
                p = write_usize(p, idx);
                p = buf_write(p, r##"" placeholder="Amount" min="1"><button class="spend-btn" onclick="addSpend("##);
                p = write_usize(p, idx);
                p = buf_write(p, r##")">+ Spend</button></div></div>"##);
                idx += 1;
            }
        }
        cpos = cend + 1;
    }

    p = buf_write(p, r##"</div>
<script>
function addCat(){var c=document.getElementById('category').value;var b=document.getElementById('budget').value;if(!c||!b)return alert('Fill in category and budget');fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'add',category:c,budget:b,spent:'0'})}).then(function(){location.reload()})}
function addSpend(i){var el=document.getElementById('spend-'+i);var a=el.value;if(!a||a==='0')return alert('Enter an amount');fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'spend',index:String(i),amount:a})}).then(function(){location.reload()})}
</script></body></html>"##);

    respond(200, buf_as_str(p), "text/html");
}
