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

// Services stored: svc_count, svc_{i}_name, svc_{i}_status (up/down/degraded), svc_{i}_uptime
// Incidents stored: incident_log = "msg1;msg2;..."
fn make_svc_key(id: u32, suffix: &str) -> &'static str {
    static mut SKBUF: [u8; 48] = [0u8; 48];
    let prefix = b"svc_";
    let id_s = num_to_str(id);
    let ib = id_s.as_bytes();
    let sb = suffix.as_bytes();
    let mut pos = 0;
    unsafe {
        SKBUF[..prefix.len()].copy_from_slice(prefix);
        pos += prefix.len();
        SKBUF[pos..pos+ib.len()].copy_from_slice(ib);
        pos += ib.len();
        SKBUF[pos] = b'_'; pos += 1;
        SKBUF[pos..pos+sb.len()].copy_from_slice(sb);
        pos += sb.len();
        core::str::from_utf8_unchecked(&SKBUF[..pos])
    }
}

fn render_dashboard() {
    let svc_count = parse_u32(kv_read("svc_count").unwrap_or("0"));
    let mut up_count: u32 = 0;
    let mut down_count: u32 = 0;
    let mut degraded_count: u32 = 0;

    // Count statuses
    let mut si: u32 = 0;
    while si < svc_count {
        let status = kv_read(make_svc_key(si, "status")).unwrap_or("up");
        match status {
            "down" => down_count += 1,
            "degraded" => degraded_count += 1,
            _ => up_count += 1,
        }
        si += 1;
    }

    let overall = if down_count > 0 { "Major Outage" }
        else if degraded_count > 0 { "Degraded Performance" }
        else if svc_count == 0 { "No Services" }
        else { "All Systems Operational" };
    let overall_color = if down_count > 0 { "#e74c3c" }
        else if degraded_count > 0 { "#f39c12" }
        else { "#2ecc71" };

    let mut p = 0;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><title>Status Dashboard</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:'Segoe UI',sans-serif;background:#0f1923;color:#e0e0e0;min-height:100vh}
.container{max-width:900px;margin:0 auto;padding:20px}
.overall{border-radius:12px;padding:25px;text-align:center;margin-bottom:25px;font-size:1.3em;font-weight:600}
h1{color:#fff;text-align:center;margin-bottom:5px;font-size:1.8em}
.subtitle{color:#8b949e;text-align:center;margin-bottom:25px}
.stats-row{display:flex;gap:15px;margin-bottom:25px}
.stat-card{flex:1;background:#1a2332;border-radius:10px;padding:20px;text-align:center;border:1px solid #2d3748}
.stat-card .num{font-size:2.5em;font-weight:bold}
.stat-card .lbl{color:#8b949e;margin-top:5px}
.services{background:#1a2332;border:1px solid #2d3748;border-radius:12px;overflow:hidden;margin-bottom:25px}
.services h2{padding:15px 20px;border-bottom:1px solid #2d3748;color:#58a6ff}
.svc-row{display:flex;align-items:center;padding:15px 20px;border-bottom:1px solid #1e2d3d}
.svc-row:last-child{border-bottom:none}
.svc-row:hover{background:#1e2d3d}
.svc-name{flex:1;font-weight:600;font-size:1.05em}
.svc-status{padding:5px 15px;border-radius:20px;font-size:0.85em;font-weight:600;text-transform:uppercase}
.svc-status.up{background:rgba(46,204,113,0.15);color:#2ecc71}
.svc-status.down{background:rgba(231,76,60,0.15);color:#e74c3c}
.svc-status.degraded{background:rgba(243,156,18,0.15);color:#f39c12}
.svc-uptime{color:#8b949e;margin-left:15px;min-width:80px;text-align:right}
.uptime-bar{height:8px;background:#2d3748;border-radius:4px;margin-top:5px;overflow:hidden;width:100px}
.uptime-fill{height:100%;border-radius:4px}
.svc-actions{display:flex;gap:5px;margin-left:10px}
.svc-actions button{background:#2d3748;color:#c9d1d9;border:none;padding:4px 10px;border-radius:4px;cursor:pointer;font-size:0.8em}
.svc-actions button:hover{background:#484f58}
.incidents{background:#1a2332;border:1px solid #2d3748;border-radius:12px;padding:20px}
.incidents h2{color:#f39c12;margin-bottom:15px}
.incident{padding:10px;border-left:3px solid #f39c12;margin-bottom:10px;background:#1e2d3d;border-radius:0 5px 5px 0}
.add-section{background:#1a2332;border:1px solid #2d3748;border-radius:12px;padding:20px;margin-bottom:25px}
.add-section h2{color:#58a6ff;margin-bottom:15px}
.add-row{display:flex;gap:10px;flex-wrap:wrap}
.add-row input,.add-row select{background:#0f1923;color:#c9d1d9;border:1px solid #2d3748;padding:8px 12px;border-radius:5px;font-size:1em}
.add-row input{flex:1;min-width:200px}
.btn{background:#238636;color:#fff;border:none;padding:8px 20px;border-radius:5px;cursor:pointer;font-weight:600}
.btn:hover{background:#2ea043}
</style></head><body>
<div class="container">
<h1>Status Dashboard</h1>
<p class="subtitle">System health monitoring</p>
<div class="overall" style="background:"##);
    p = buf_write(p, overall_color);
    p = buf_write(p, r##";color:#fff">"##);
    p = buf_write(p, overall);
    p = buf_write(p, r##"</div>
<div class="stats-row">
<div class="stat-card"><div class="num" style="color:#2ecc71">"##);
    p = buf_write(p, num_to_str(up_count));
    p = buf_write(p, r##"</div><div class="lbl">Operational</div></div>
<div class="stat-card"><div class="num" style="color:#f39c12">"##);
    p = buf_write(p, num_to_str(degraded_count));
    p = buf_write(p, r##"</div><div class="lbl">Degraded</div></div>
<div class="stat-card"><div class="num" style="color:#e74c3c">"##);
    p = buf_write(p, num_to_str(down_count));
    p = buf_write(p, r##"</div><div class="lbl">Down</div></div>
<div class="stat-card"><div class="num" style="color:#58a6ff">"##);
    p = buf_write(p, num_to_str(svc_count));
    p = buf_write(p, r##"</div><div class="lbl">Total</div></div>
</div>
<div class="add-section"><h2>Add Service / Log Incident</h2>
<div class="add-row" style="margin-bottom:10px">
<input type="text" id="svc-name" placeholder="Service name">
<button class="btn" onclick="addService()">Add Service</button>
</div>
<div class="add-row">
<input type="text" id="incident-msg" placeholder="Incident description">
<button class="btn" style="background:#f39c12" onclick="logIncident()">Log Incident</button>
</div>
</div>
<div class="services"><h2>Services</h2>"##);

    si = 0;
    while si < svc_count {
        let name = kv_read(make_svc_key(si, "name")).unwrap_or("Unknown");
        let status = kv_read(make_svc_key(si, "status")).unwrap_or("up");
        let uptime = kv_read(make_svc_key(si, "uptime")).unwrap_or("100");
        let uptime_n = parse_u32(uptime);
        let bar_color = if uptime_n > 95 { "#2ecc71" } else if uptime_n > 80 { "#f39c12" } else { "#e74c3c" };
        p = buf_write(p, r##"<div class="svc-row"><span class="svc-name">"##);
        p = buf_write(p, name);
        p = buf_write(p, r##"</span><span class="svc-status "##);
        p = buf_write(p, status);
        p = buf_write(p, r##"">"##);
        p = buf_write(p, status);
        p = buf_write(p, r##"</span><div class="svc-uptime">"##);
        p = buf_write(p, uptime);
        p = buf_write(p, r##"%<div class="uptime-bar"><div class="uptime-fill" style="width:"##);
        p = buf_write(p, uptime);
        p = buf_write(p, r##"%;background:"##);
        p = buf_write(p, bar_color);
        p = buf_write(p, r##""></div></div></div>
<div class="svc-actions">
<button onclick="setStatus("##);
        p = buf_write(p, num_to_str(si));
        p = buf_write(p, r##",'up')">Up</button>
<button onclick="setStatus("##);
        p = buf_write(p, num_to_str(si));
        p = buf_write(p, r##",'degraded')">Deg</button>
<button onclick="setStatus("##);
        p = buf_write(p, num_to_str(si));
        p = buf_write(p, r##",'down')">Down</button>
<button onclick="removeSvc("##);
        p = buf_write(p, num_to_str(si));
        p = buf_write(p, r##")">Del</button>
</div></div>"##);
        si += 1;
    }
    if svc_count == 0 {
        p = buf_write(p, r##"<div style="padding:30px;text-align:center;color:#8b949e">No services configured. Add one above.</div>"##);
    }

    p = buf_write(p, r##"</div>
<div class="incidents"><h2>Incident Log</h2>"##);
    let incidents = kv_read("incident_log").unwrap_or("");
    if incidents.is_empty() {
        p = buf_write(p, r##"<p style="color:#8b949e">No incidents recorded.</p>"##);
    } else {
        let ib = incidents.as_bytes();
        let mut es = 0;
        let mut ei = 0;
        while ei <= ib.len() {
            if ei == ib.len() || ib[ei] == b';' {
                if ei > es {
                    if let Ok(inc) = core::str::from_utf8(&ib[es..ei]) {
                        p = buf_write(p, r##"<div class="incident">"##);
                        p = buf_write(p, inc);
                        p = buf_write(p, r##"</div>"##);
                    }
                }
                es = ei + 1;
            }
            ei += 1;
        }
    }
    p = buf_write(p, r##"</div></div>
<script>
function addService(){
  const n=document.getElementById('svc-name').value;
  if(!n)return;
  fetch('',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({action:'add_svc',name:n})}).then(()=>location.reload());
}
function setStatus(id,s){
  fetch('',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({action:'set_status',id:''+id,status:s})}).then(()=>location.reload());
}
function removeSvc(id){
  if(!confirm('Remove service?'))return;
  fetch('',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({action:'remove_svc',id:''+id})}).then(()=>location.reload());
}
function logIncident(){
  const m=document.getElementById('incident-msg').value;
  if(!m)return;
  fetch('',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({action:'incident',message:m})}).then(()=>location.reload());
}
</script></body></html>"##);
    respond(200, buf_as_str(p), "text/html");
}

#[no_mangle]
pub extern "C" fn x402_handle(method_ptr: *const u8, method_len: i32, path_ptr: *const u8, path_len: i32, body_ptr: *const u8, body_len: i32) {
    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };
    let body = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(body_ptr, body_len as usize)) };

    host_log(1, "Status dashboard request");

    if method == "POST" {
        let action = find_json_str(body, "action").unwrap_or("");

        if action == "add_svc" {
            let name = find_json_str(body, "name").unwrap_or("Service");
            let count = parse_u32(kv_read("svc_count").unwrap_or("0"));
            kv_write(make_svc_key(count, "name"), name);
            kv_write(make_svc_key(count, "status"), "up");
            kv_write(make_svc_key(count, "uptime"), "100");
            kv_write("svc_count", num_to_str(count + 1));
            respond(200, r##"{"ok":true}"##, "application/json");
        } else if action == "set_status" {
            let id = parse_u32(find_json_str(body, "id").unwrap_or("0"));
            let status = find_json_str(body, "status").unwrap_or("up");
            kv_write(make_svc_key(id, "status"), status);
            // Simulate uptime impact
            let current_uptime = parse_u32(kv_read(make_svc_key(id, "uptime")).unwrap_or("100"));
            let new_uptime = match status {
                "down" => if current_uptime > 5 { current_uptime - 5 } else { 0 },
                "degraded" => if current_uptime > 2 { current_uptime - 2 } else { 0 },
                _ => if current_uptime < 99 { current_uptime + 1 } else { 100 },
            };
            kv_write(make_svc_key(id, "uptime"), num_to_str(new_uptime));
            respond(200, r##"{"ok":true}"##, "application/json");
        } else if action == "remove_svc" {
            // Mark as removed by clearing name
            let id = parse_u32(find_json_str(body, "id").unwrap_or("0"));
            kv_write(make_svc_key(id, "name"), "(removed)");
            kv_write(make_svc_key(id, "status"), "up");
            respond(200, r##"{"ok":true}"##, "application/json");
        } else if action == "incident" {
            let msg = find_json_str(body, "message").unwrap_or("Incident");
            let existing = kv_read("incident_log").unwrap_or("");
            if existing.is_empty() {
                kv_write("incident_log", msg);
            } else {
                let mut nbuf = [0u8; 4096];
                let mb = msg.as_bytes();
                nbuf[..mb.len()].copy_from_slice(mb);
                nbuf[mb.len()] = b';';
                let eb = existing.as_bytes();
                let copy_len = eb.len().min(4096 - mb.len() - 1);
                nbuf[mb.len()+1..mb.len()+1+copy_len].copy_from_slice(&eb[..copy_len]);
                let nv = unsafe { core::str::from_utf8_unchecked(&nbuf[..mb.len()+1+copy_len]) };
                kv_write("incident_log", nv);
            }
            respond(200, r##"{"ok":true}"##, "application/json");
        } else {
            respond(400, r##"{"error":"unknown"}"##, "application/json");
        }
        return;
    }

    render_dashboard();
}