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

static mut NBUF: [u8; 32] = [0u8; 32];
fn num_to_str(mut n: u32) -> &'static str {
    if n == 0 {
        unsafe { NBUF[0] = b'0'; }
        return unsafe { core::str::from_utf8_unchecked(&NBUF[..1]) };
    }
    let mut i = 31;
    while n > 0 {
        unsafe { NBUF[i] = b'0' + (n % 10) as u8; }
        n /= 10;
        i -= 1;
    }
    unsafe { core::str::from_utf8_unchecked(&NBUF[i + 1..32]) }
}

fn parse_u32(s: &str) -> u32 {
    let mut n: u32 = 0;
    for &b in s.as_bytes() {
        if b >= b'0' && b <= b'9' {
            n = n.wrapping_mul(10).wrapping_add((b - b'0') as u32);
        }
    }
    n
}

fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(month: u32, year: u32) -> u32 {
    match month {
        1 => 31, 2 => if is_leap_year(year) { 29 } else { 28 },
        3 => 31, 4 => 30, 5 => 31, 6 => 30,
        7 => 31, 8 => 31, 9 => 30, 10 => 31, 11 => 30, 12 => 31,
        _ => 30,
    }
}

// Zeller's congruence: day of week for first day of month (0=Sun,1=Mon,...6=Sat)
fn first_day_of_month(month: u32, year: u32) -> u32 {
    let m = if month < 3 { month + 12 } else { month };
    let y = if month < 3 { year - 1 } else { year };
    let q: u32 = 1;
    let k = y % 100;
    let j = y / 100;
    let h = (q + (13 * (m + 1)) / 5 + k + k / 4 + j / 4 + 5 * j) % 7;
    // h: 0=Sat,1=Sun,2=Mon,...6=Fri -> convert to 0=Sun
    ((h + 6) % 7)
}

fn month_name(m: u32) -> &'static str {
    match m {
        1 => "January", 2 => "February", 3 => "March", 4 => "April",
        5 => "May", 6 => "June", 7 => "July", 8 => "August",
        9 => "September", 10 => "October", 11 => "November", 12 => "December",
        _ => "Unknown",
    }
}

// Events stored as: cal_events_{year}_{month}_{day} = "event1;event2;..."
fn make_day_key(year: u32, month: u32, day: u32) -> &'static str {
    static mut DKBUF: [u8; 32] = [0u8; 32];
    let prefix = b"cal_";
    let ys = num_to_str(year);
    let ms = num_to_str(month);
    let ds = num_to_str(day);
    let yb = ys.as_bytes();
    let mb = ms.as_bytes();
    let db = ds.as_bytes();
    let mut pos = 0;
    unsafe {
        DKBUF[..prefix.len()].copy_from_slice(prefix);
        pos += prefix.len();
        DKBUF[pos..pos+yb.len()].copy_from_slice(yb);
        pos += yb.len();
        DKBUF[pos] = b'_'; pos += 1;
        DKBUF[pos..pos+mb.len()].copy_from_slice(mb);
        pos += mb.len();
        DKBUF[pos] = b'_'; pos += 1;
        DKBUF[pos..pos+db.len()].copy_from_slice(db);
        pos += db.len();
        core::str::from_utf8_unchecked(&DKBUF[..pos])
    }
}

fn find_query_param<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    let kb = key.as_bytes();
    let qb = query.as_bytes();
    let mut i = 0;
    while i + kb.len() < qb.len() {
        if (i == 0 || qb[i - 1] == b'&' || qb[i - 1] == b'?') && &qb[i..i + kb.len()] == kb && i + kb.len() < qb.len() && qb[i + kb.len()] == b'=' {
            let vs = i + kb.len() + 1;
            let mut ve = vs;
            while ve < qb.len() && qb[ve] != b'&' { ve += 1; }
            return core::str::from_utf8(&qb[vs..ve]).ok();
        }
        i += 1;
    }
    None
}

fn render_calendar(year: u32, month: u32) {
    let dim = days_in_month(month, year);
    let start_dow = first_day_of_month(month, year);
    let prev_month = if month == 1 { 12 } else { month - 1 };
    let prev_year = if month == 1 { year - 1 } else { year };
    let next_month = if month == 12 { 1 } else { month + 1 };
    let next_year = if month == 12 { year + 1 } else { year };

    let mut p = 0;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><title>Event Calendar</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:'Segoe UI',sans-serif;background:linear-gradient(135deg,#667eea 0%,#764ba2 100%);min-height:100vh;padding:20px;display:flex;justify-content:center}
.container{max-width:900px;width:100%}
.header{background:rgba(255,255,255,0.15);backdrop-filter:blur(10px);border-radius:15px;padding:20px;display:flex;justify-content:space-between;align-items:center;margin-bottom:20px}
.header h1{color:#fff;font-size:1.5em}
.nav-btn{background:rgba(255,255,255,0.2);color:#fff;border:none;padding:10px 20px;border-radius:8px;cursor:pointer;font-size:1em;font-weight:600}
.nav-btn:hover{background:rgba(255,255,255,0.3)}
.month-title{color:#fff;font-size:1.3em;font-weight:600}
.calendar{background:rgba(255,255,255,0.95);border-radius:15px;overflow:hidden;box-shadow:0 20px 60px rgba(0,0,0,0.3)}
.dow-row{display:grid;grid-template-columns:repeat(7,1fr);background:#f0f0f0}
.dow-cell{padding:12px;text-align:center;font-weight:600;color:#555;font-size:0.85em}
.days{display:grid;grid-template-columns:repeat(7,1fr)}
.day-cell{min-height:100px;border:1px solid #e8e8e8;padding:5px;position:relative;cursor:pointer;transition:background 0.2s}
.day-cell:hover{background:#f5f0ff}
.day-cell.empty{background:#fafafa;cursor:default}
.day-cell.empty:hover{background:#fafafa}
.day-num{font-weight:600;color:#333;font-size:0.95em;margin-bottom:3px}
.day-cell.today .day-num{background:#667eea;color:#fff;width:28px;height:28px;border-radius:50%;display:flex;align-items:center;justify-content:center}
.event{background:#667eea;color:#fff;padding:2px 6px;border-radius:3px;font-size:0.75em;margin-bottom:2px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;cursor:pointer}
.event:nth-child(3n+1){background:#e74c3c}
.event:nth-child(3n+2){background:#3498db}
.event:nth-child(3n){background:#2ecc71}
.modal{display:none;position:fixed;top:0;left:0;right:0;bottom:0;background:rgba(0,0,0,0.6);z-index:100;align-items:center;justify-content:center}
.modal-box{background:#fff;border-radius:12px;padding:30px;max-width:400px;width:90%}
.modal-box h3{color:#333;margin-bottom:15px}
.modal-box input{width:100%;padding:10px;border:2px solid #ddd;border-radius:6px;font-size:1em;margin-bottom:10px}
.modal-box select{padding:8px;border:2px solid #ddd;border-radius:6px;margin-bottom:15px}
.btn{background:#667eea;color:#fff;border:none;padding:10px 20px;border-radius:6px;cursor:pointer;font-size:1em;font-weight:600}
.btn:hover{background:#5a6fd6}
.btn-danger{background:#e74c3c}
.btn-danger:hover{background:#c0392b}
.event-list{margin-top:10px}
.event-item{display:flex;justify-content:space-between;align-items:center;padding:8px;background:#f5f0ff;border-radius:5px;margin-bottom:5px}
</style></head><body>
<div class="container">
<div class="header">
<button class="nav-btn" onclick="navMonth("##);
    p = buf_write(p, num_to_str(prev_year));
    p = buf_write(p, ",");
    p = buf_write(p, num_to_str(prev_month));
    p = buf_write(p, r##")">&#9664; Prev</button>
<div class="month-title">"##);
    p = buf_write(p, month_name(month));
    p = buf_write(p, " ");
    p = buf_write(p, num_to_str(year));
    p = buf_write(p, r##"</div>
<button class="nav-btn" onclick="navMonth("##);
    p = buf_write(p, num_to_str(next_year));
    p = buf_write(p, ",");
    p = buf_write(p, num_to_str(next_month));
    p = buf_write(p, r##")">Next &#9654;</button>
</div>
<div class="calendar">
<div class="dow-row">
<div class="dow-cell">Sun</div><div class="dow-cell">Mon</div><div class="dow-cell">Tue</div>
<div class="dow-cell">Wed</div><div class="dow-cell">Thu</div><div class="dow-cell">Fri</div><div class="dow-cell">Sat</div>
</div>
<div class="days">"##);

    // Empty cells before first day
    let mut cell: u32 = 0;
    while cell < start_dow {
        p = buf_write(p, r##"<div class="day-cell empty"></div>"##);
        cell += 1;
    }

    // Day cells
    let mut day: u32 = 1;
    while day <= dim {
        let day_key = make_day_key(year, month, day);
        let events = kv_read(day_key).unwrap_or("");
        p = buf_write(p, r##"<div class="day-cell" onclick="openDay("##);
        p = buf_write(p, num_to_str(day));
        p = buf_write(p, r##")"><div class="day-num">"##);
        p = buf_write(p, num_to_str(day));
        p = buf_write(p, r##"</div>"##);

        // Render events for this day
        if !events.is_empty() {
            let eb = events.as_bytes();
            let mut es = 0;
            let mut ei = 0;
            while ei <= eb.len() {
                if ei == eb.len() || eb[ei] == b';' {
                    if ei > es {
                        if let Ok(ev) = core::str::from_utf8(&eb[es..ei]) {
                            p = buf_write(p, r##"<div class="event">"##);
                            p = buf_write(p, ev);
                            p = buf_write(p, r##"</div>"##);
                        }
                    }
                    es = ei + 1;
                }
                ei += 1;
            }
        }
        p = buf_write(p, r##"</div>"##);
        day += 1;
    }

    p = buf_write(p, r##"</div></div></div>
<div class="modal" id="day-modal">
<div class="modal-box">
<h3 id="modal-title">Day</h3>
<div id="modal-events" class="event-list"></div>
<hr style="margin:15px 0;border-color:#eee">
<input type="text" id="new-event" placeholder="New event name...">
<div style="display:flex;gap:10px">
<button class="btn" onclick="addEvent()">Add Event</button>
<button class="btn" style="background:#999" onclick="closeModal()">Close</button>
</div>
</div></div>
<script>
let curYear="##);
    p = buf_write(p, num_to_str(year));
    p = buf_write(p, r##",curMonth="##);
    p = buf_write(p, num_to_str(month));
    p = buf_write(p, r##",curDay=1;
function navMonth(y,m){location.href='?year='+y+'&month='+m;}
function openDay(d){
  curDay=d;
  document.getElementById('modal-title').textContent='"##);
    p = buf_write(p, month_name(month));
    p = buf_write(p, r##" '+d+', "##);
    p = buf_write(p, num_to_str(year));
    p = buf_write(p, r##"';
  fetch('?action=getday&year='+curYear+'&month='+curMonth+'&day='+d)
  .then(r=>r.json()).then(data=>{
    let html='';
    if(data.events){
      data.events.forEach((e,i)=>{
        html+='<div class="event-item"><span>'+e+'</span><button class="btn btn-danger" style="padding:3px 10px;font-size:0.8em" onclick="delEvent('+i+')">Delete</button></div>';
      });
    }
    document.getElementById('modal-events').innerHTML=html||'<p style="color:#999">No events</p>';
  });
  document.getElementById('day-modal').style.display='flex';
}
function closeModal(){document.getElementById('day-modal').style.display='none';}
function addEvent(){
  const name=document.getElementById('new-event').value;
  if(!name)return;
  fetch('',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({action:'add',year:''+curYear,month:''+curMonth,day:''+curDay,event:name})})
  .then(()=>{document.getElementById('new-event').value='';openDay(curDay);});
}
function delEvent(idx){
  fetch('',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({action:'delete',year:''+curYear,month:''+curMonth,day:''+curDay,idx:''+idx})})
  .then(()=>openDay(curDay));
}
</script></body></html>"##);
    respond(200, buf_as_str(p), "text/html");
}

#[no_mangle]
pub extern "C" fn x402_handle(method_ptr: *const u8, method_len: i32, path_ptr: *const u8, path_len: i32, body_ptr: *const u8, body_len: i32) {
    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };
    let path = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(path_ptr, path_len as usize)) };
    let body = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(body_ptr, body_len as usize)) };

    host_log(1, "Calendar request");

    let query = if let Some(qi) = path.as_bytes().iter().position(|&b| b == b'?') {
        &path[qi + 1..]
    } else {
        ""
    };

    if method == "POST" {
        let action = find_json_str(body, "action").unwrap_or("");
        let year = parse_u32(find_json_str(body, "year").unwrap_or("2026"));
        let month = parse_u32(find_json_str(body, "month").unwrap_or("1"));
        let day = parse_u32(find_json_str(body, "day").unwrap_or("1"));
        let day_key = make_day_key(year, month, day);

        if action == "add" {
            let event = find_json_str(body, "event").unwrap_or("Event");
            let existing = kv_read(day_key).unwrap_or("");
            if existing.is_empty() {
                kv_write(day_key, event);
            } else {
                let mut nbuf = [0u8; 2048];
                let eb = existing.as_bytes();
                nbuf[..eb.len()].copy_from_slice(eb);
                nbuf[eb.len()] = b';';
                let evb = event.as_bytes();
                nbuf[eb.len()+1..eb.len()+1+evb.len()].copy_from_slice(evb);
                let nv = unsafe { core::str::from_utf8_unchecked(&nbuf[..eb.len()+1+evb.len()]) };
                kv_write(day_key, nv);
            }
            respond(200, r##"{"ok":true}"##, "application/json");
        } else if action == "delete" {
            let idx = parse_u32(find_json_str(body, "idx").unwrap_or("0"));
            let existing = kv_read(day_key).unwrap_or("");
            let eb = existing.as_bytes();
            let mut result = [0u8; 2048];
            let mut rp = 0;
            let mut es = 0;
            let mut ei = 0;
            let mut cur_idx: u32 = 0;
            let mut first = true;
            while ei <= eb.len() {
                if ei == eb.len() || eb[ei] == b';' {
                    if cur_idx != idx && ei > es {
                        if !first { result[rp] = b';'; rp += 1; }
                        result[rp..rp+(ei-es)].copy_from_slice(&eb[es..ei]);
                        rp += ei - es;
                        first = false;
                    }
                    cur_idx += 1;
                    es = ei + 1;
                }
                ei += 1;
            }
            let nv = unsafe { core::str::from_utf8_unchecked(&result[..rp]) };
            kv_write(day_key, nv);
            respond(200, r##"{"ok":true}"##, "application/json");
        } else {
            respond(400, r##"{"error":"unknown"}"##, "application/json");
        }
        return;
    }

    // GET: check for getday action
    if let Some(_) = find_query_param(query, "action") {
        let year = parse_u32(find_query_param(query, "year").unwrap_or("2026"));
        let month = parse_u32(find_query_param(query, "month").unwrap_or("1"));
        let day = parse_u32(find_query_param(query, "day").unwrap_or("1"));
        let day_key = make_day_key(year, month, day);
        let events = kv_read(day_key).unwrap_or("");
        let mut rp = 0;
        rp = buf_write(rp, r##"{"events":["##);
        if !events.is_empty() {
            let eb = events.as_bytes();
            let mut es = 0;
            let mut ei = 0;
            let mut first = true;
            while ei <= eb.len() {
                if ei == eb.len() || eb[ei] == b';' {
                    if ei > es {
                        if !first { rp = buf_write(rp, ","); }
                        rp = buf_write(rp, r##"""##);
                        if let Ok(ev) = core::str::from_utf8(&eb[es..ei]) {
                            rp = buf_write(rp, ev);
                        }
                        rp = buf_write(rp, r##"""##);
                        first = false;
                    }
                    es = ei + 1;
                }
                ei += 1;
            }
        }
        rp = buf_write(rp, "]}");
        respond(200, buf_as_str(rp), "application/json");
        return;
    }

    let year = parse_u32(find_query_param(query, "year").unwrap_or("2026"));
    let month = parse_u32(find_query_param(query, "month").unwrap_or("4"));
    let year = if year < 2000 || year > 2100 { 2026 } else { year };
    let month = if month < 1 || month > 12 { 4 } else { month };
    render_calendar(year, month);
}