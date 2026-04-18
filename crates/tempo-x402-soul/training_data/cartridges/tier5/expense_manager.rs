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

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(request_ptr, request_len as usize)) };
    let method = find_json_str(request, "method").unwrap_or("GET");
    let body = find_json_str(request, "body").unwrap_or("");
    host_log(0, "expense_manager: handling request");

    if method == "POST" {
        if let Some(action) = find_json_str(body, "action") {
            if action == "add" {
                let desc = find_json_str(body, "desc").unwrap_or("");
                let amount = find_json_str(body, "amount").unwrap_or("0");
                let category = find_json_str(body, "category").unwrap_or("other");
                let date = find_json_str(body, "date").unwrap_or("");
                let exp_type = find_json_str(body, "type").unwrap_or("expense");
                if !desc.is_empty() {
                    let existing = kv_read("exp_data").unwrap_or("");
                    let mut w = W::new();
                    if !existing.is_empty() { w.s(existing); w.s("\n"); }
                    w.s(date); w.s("|"); w.s(desc); w.s("|"); w.s(amount); w.s("|"); w.s(category); w.s("|"); w.s(exp_type);
                    kv_write("exp_data", w.out());
                    respond(200, r#"{"ok":true}"#, "application/json");
                } else { respond(400, r#"{"error":"desc required"}"#, "application/json"); }
            } else if action == "set_budget" {
                if let Some(budget) = find_json_str(body, "budget") {
                    kv_write("exp_budget", budget);
                    respond(200, r#"{"ok":true}"#, "application/json");
                } else { respond(400, r#"{"error":"missing budget"}"#, "application/json"); }
            } else if action == "delete" {
                let idx = find_json_str(body, "index").map(|s| parse_u32(s)).unwrap_or(0);
                let existing = kv_read("exp_data").unwrap_or("");
                let mut w = W::new();
                let eb = existing.as_bytes();
                let mut p = 0; let mut line_num: u32 = 0;
                while p < eb.len() {
                    let ls = p;
                    while p < eb.len() && eb[p] != b'\n' { p += 1; }
                    let line = unsafe { core::str::from_utf8_unchecked(&eb[ls..p]) };
                    if p < eb.len() { p += 1; }
                    if line_num != idx && !line.is_empty() {
                        if w.pos > 0 { w.s("\n"); }
                        w.s(line);
                    }
                    line_num += 1;
                }
                kv_write("exp_data", w.out());
                respond(200, r#"{"ok":true}"#, "application/json");
            } else { respond(400, r#"{"error":"unknown"}"#, "application/json"); }
        } else { respond(400, r#"{"error":"missing action"}"#, "application/json"); }
        return;
    }

    let data = kv_read("exp_data").unwrap_or("");
    let budget = kv_read("exp_budget").map(|s| parse_u32(s)).unwrap_or(1000);

    // Compute totals by category
    let categories = ["food", "transport", "housing", "utilities", "entertainment", "health", "shopping", "other"];
    let mut cat_totals = [0u32; 8];
    let mut total_expense: u32 = 0;
    let mut total_income: u32 = 0;

    let db = data.as_bytes();
    let mut p = 0;
    while p < db.len() {
        let ls = p;
        while p < db.len() && db[p] != b'\n' { p += 1; }
        let line = unsafe { core::str::from_utf8_unchecked(&db[ls..p]) };
        if p < db.len() { p += 1; }
        let lb = line.as_bytes();
        let mut pipes = [0usize; 4]; let mut pi = 0; let mut li = 0;
        while li < lb.len() && pi < 4 { if lb[li] == b'|' { pipes[pi] = li; pi += 1; } li += 1; }
        if pi >= 4 {
            let amount = parse_u32(&line[pipes[1]+1..pipes[2]]);
            let category = &line[pipes[2]+1..pipes[3]];
            let exp_type = &line[pipes[3]+1..];
            if exp_type == "income" {
                total_income += amount;
            } else {
                total_expense += amount;
                let mut ci = 0;
                while ci < categories.len() {
                    if categories[ci] == category { cat_totals[ci] += amount; }
                    ci += 1;
                }
            }
        }
    }

    let balance = if total_income >= total_expense { total_income - total_expense } else { 0 };
    let budget_pct = if budget > 0 { (total_expense * 100) / budget } else { 0 };
    let budget_pct_c = if budget_pct > 100 { 100 } else { budget_pct };

    let mut w = W::new();
    w.s("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Expense Manager</title><style>");
    w.s("*{margin:0;padding:0;box-sizing:border-box}body{background:#0d1117;color:#c9d1d9;font-family:'Segoe UI',sans-serif;padding:30px 20px;display:flex;justify-content:center}");
    w.s(".c{max-width:700px;width:100%}h1{text-align:center;color:#58a6ff;margin-bottom:20px}");
    w.s(".summary{display:grid;grid-template-columns:repeat(3,1fr);gap:12px;margin-bottom:20px}");
    w.s(".sum-card{background:#161b22;padding:16px;border-radius:10px;text-align:center}");
    w.s(".sum-card .val{font-size:24px;font-weight:bold;margin-bottom:4px}.sum-card .label{font-size:12px;color:#8b949e;text-transform:uppercase}");
    w.s(".sum-card.income .val{color:#3fb950}.sum-card.expense .val{color:#f85149}.sum-card.balance .val{color:#58a6ff}");
    w.s(".budget-bar{background:#161b22;padding:16px;border-radius:10px;margin-bottom:20px}");
    w.s(".budget-bar .track{height:12px;background:#21262d;border-radius:6px;overflow:hidden;margin:8px 0}");
    w.s(".budget-bar .fill{height:100%;border-radius:6px;transition:width 0.3s}");
    w.s(".budget-bar .info{display:flex;justify-content:space-between;font-size:13px;color:#8b949e}");
    w.s(".form{background:#161b22;padding:16px;border-radius:10px;margin-bottom:20px;display:flex;gap:8px;flex-wrap:wrap}");
    w.s("input,select{padding:10px;background:#0d1117;border:1px solid #30363d;color:#c9d1d9;border-radius:6px;font-size:14px}");
    w.s("input{flex:1;min-width:100px}select{width:auto}");
    w.s("button{padding:10px 18px;background:#238636;color:#fff;border:none;border-radius:6px;cursor:pointer;font-size:14px}");
    w.s(".categories{display:grid;grid-template-columns:repeat(2,1fr);gap:8px;margin-bottom:20px}");
    w.s(".cat-card{background:#161b22;padding:12px;border-radius:8px;display:flex;justify-content:space-between;align-items:center}");
    w.s(".cat-card .name{font-size:14px;color:#8b949e;text-transform:capitalize}.cat-card .amt{font-size:16px;font-weight:bold;color:#f85149}");
    w.s(".transactions{max-height:400px;overflow-y:auto}");
    w.s(".tx{display:flex;align-items:center;gap:12px;padding:10px 12px;background:#161b22;border-radius:6px;margin-bottom:4px}");
    w.s(".tx .date{width:80px;font-size:12px;color:#8b949e}.tx .desc{flex:1;font-size:14px}");
    w.s(".tx .cat{font-size:11px;color:#58a6ff;background:#0d2744;padding:2px 8px;border-radius:8px}");
    w.s(".tx .amt{font-weight:bold;min-width:60px;text-align:right}.tx .amt.exp{color:#f85149}.tx .amt.inc{color:#3fb950}");
    w.s(".tx .del{background:none;border:none;color:#666;cursor:pointer;font-size:14px;padding:4px}");
    w.s("</style></head><body><div class='c'><h1>Expense Manager</h1>");

    // Summary cards
    w.s("<div class='summary'><div class='sum-card income'><div class='val'>$"); w.n(total_income);
    w.s("</div><div class='label'>Income</div></div><div class='sum-card expense'><div class='val'>$"); w.n(total_expense);
    w.s("</div><div class='label'>Expenses</div></div><div class='sum-card balance'><div class='val'>$"); w.n(balance);
    w.s("</div><div class='label'>Balance</div></div></div>");

    // Budget bar
    let bar_color = if budget_pct > 90 { "#f85149" } else if budget_pct > 70 { "#d29922" } else { "#3fb950" };
    w.s("<div class='budget-bar'><div class='info'><span>Budget: $"); w.n(budget);
    w.s("</span><span>"); w.n(budget_pct); w.s("% used</span></div>");
    w.s("<div class='track'><div class='fill' style='width:"); w.n(budget_pct_c);
    w.s("%;background:"); w.s(bar_color); w.s("'></div></div></div>");

    // Add form
    w.s("<div class='form'><input type='date' id='date'><input id='desc' placeholder='Description'><input type='number' id='amt' placeholder='Amount'>");
    w.s("<select id='cat'><option value='food'>Food</option><option value='transport'>Transport</option><option value='housing'>Housing</option><option value='utilities'>Utilities</option><option value='entertainment'>Entertainment</option><option value='health'>Health</option><option value='shopping'>Shopping</option><option value='other'>Other</option></select>");
    w.s("<select id='type'><option value='expense'>Expense</option><option value='income'>Income</option></select>");
    w.s("<button onclick='add()'>Add</button></div>");

    // Category breakdown
    w.s("<div class='categories'>");
    let mut ci = 0;
    while ci < categories.len() {
        if cat_totals[ci] > 0 {
            w.s("<div class='cat-card'><span class='name'>"); w.s(categories[ci]);
            w.s("</span><span class='amt'>$"); w.n(cat_totals[ci]); w.s("</span></div>");
        }
        ci += 1;
    }
    w.s("</div>");

    // Transaction list (newest first)
    w.s("<div class='transactions'>");
    if !data.is_empty() {
        // Count lines
        let mut count: u32 = 1;
        let mut cp = 0;
        while cp < db.len() { if db[cp] == b'\n' { count += 1; } cp += 1; }

        let mut idx = count;
        let mut p2 = db.len();
        // Iterate backwards through entries
        while idx > 0 {
            idx -= 1;
            // Find start of this line
            let le = p2;
            if p2 > 0 && db[p2 - 1] == b'\n' { p2 -= 1; }
            let mut ls2 = p2;
            while ls2 > 0 && db[ls2 - 1] != b'\n' { ls2 -= 1; }
            let line = unsafe { core::str::from_utf8_unchecked(&db[ls2..p2]) };
            p2 = if ls2 > 0 { ls2 - 1 } else { 0 };

            if line.is_empty() { continue; }
            let lb = line.as_bytes();
            let mut pipes = [0usize; 4]; let mut pi = 0; let mut li = 0;
            while li < lb.len() && pi < 4 { if lb[li] == b'|' { pipes[pi] = li; pi += 1; } li += 1; }
            if pi >= 4 {
                let date = &line[..pipes[0]];
                let desc = &line[pipes[0]+1..pipes[1]];
                let amount = &line[pipes[1]+1..pipes[2]];
                let category = &line[pipes[2]+1..pipes[3]];
                let exp_type = &line[pipes[3]+1..];
                let is_income = exp_type == "income";
                w.s("<div class='tx'><span class='date'>"); w.s(date);
                w.s("</span><span class='desc'>"); w.s(desc);
                w.s("</span><span class='cat'>"); w.s(category);
                w.s("</span><span class='amt "); w.s(if is_income { "inc" } else { "exp" });
                w.s("'>"); w.s(if is_income { "+" } else { "-" }); w.s("$"); w.s(amount);
                w.s("</span><button class='del' onclick='del("); w.n(idx); w.s(")'>x</button></div>");
            }
            if idx == 0 { break; }
        }
    }
    w.s("</div></div>");

    w.s("<script>const B=location.pathname;document.getElementById('date').valueAsDate=new Date();");
    w.s("async function add(){const d=document.getElementById('date').value;const desc=document.getElementById('desc').value.trim();const a=document.getElementById('amt').value;const c=document.getElementById('cat').value;const t=document.getElementById('type').value;if(!desc||!a)return;await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'add',date:d,desc:desc,amount:a,category:c,type:t})});location.reload();}");
    w.s("async function del(i){await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'delete',index:String(i)})});location.reload();}");
    w.s("</script></body></html>");
    respond(200, w.out(), "text/html");
}
