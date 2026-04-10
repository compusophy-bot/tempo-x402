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

// Project: proj_{id}_name, proj_{id}_desc, proj_{id}_priority (high/med/low)
// Tasks: proj_{id}_tasks = "task1|done;task2|todo;..."
// proj_count = total projects

fn make_proj_key(id: u32, suffix: &str) -> &'static str {
    static mut PKBUF: [u8; 48] = [0u8; 48];
    let prefix = b"proj_";
    let id_s = num_to_str(id);
    let ib = id_s.as_bytes();
    let sb = suffix.as_bytes();
    let mut pos = 0;
    unsafe {
        PKBUF[..prefix.len()].copy_from_slice(prefix);
        pos += prefix.len();
        PKBUF[pos..pos+ib.len()].copy_from_slice(ib);
        pos += ib.len();
        PKBUF[pos] = b'_'; pos += 1;
        PKBUF[pos..pos+sb.len()].copy_from_slice(sb);
        pos += sb.len();
        core::str::from_utf8_unchecked(&PKBUF[..pos])
    }
}

fn count_tasks(tasks_str: &str) -> (u32, u32) {
    // Returns (total, done)
    if tasks_str.is_empty() { return (0, 0); }
    let tb = tasks_str.as_bytes();
    let mut total: u32 = 0;
    let mut done: u32 = 0;
    let mut es = 0;
    let mut ei = 0;
    while ei <= tb.len() {
        if ei == tb.len() || tb[ei] == b';' {
            if ei > es {
                total += 1;
                // Check if ends with |done
                let segment = &tb[es..ei];
                if segment.len() > 5 {
                    let tail = &segment[segment.len()-4..];
                    if tail == b"done" { done += 1; }
                }
            }
            es = ei + 1;
        }
        ei += 1;
    }
    (total, done)
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

fn render_project_list() {
    let proj_count = parse_u32(kv_read("proj_count").unwrap_or("0"));
    let mut p = 0;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><title>Project Tracker</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:'Segoe UI',sans-serif;background:#f0f2f5;color:#333;min-height:100vh}
.container{max-width:1000px;margin:0 auto;padding:20px}
h1{color:#1a73e8;font-size:1.8em;margin-bottom:5px}
.subtitle{color:#5f6368;margin-bottom:25px}
.add-project{background:#fff;border-radius:12px;padding:20px;margin-bottom:25px;box-shadow:0 2px 8px rgba(0,0,0,0.08)}
.add-project h2{color:#1a73e8;margin-bottom:15px;font-size:1.2em}
.form-row{display:flex;gap:10px;margin-bottom:10px;flex-wrap:wrap}
.form-row input,.form-row select,.form-row textarea{padding:10px;border:2px solid #e0e0e0;border-radius:8px;font-size:1em;flex:1;min-width:150px}
.form-row textarea{height:60px;resize:vertical}
.btn{background:#1a73e8;color:#fff;border:none;padding:10px 20px;border-radius:8px;cursor:pointer;font-weight:600;font-size:1em}
.btn:hover{background:#1557b0}
.btn-danger{background:#ea4335}
.btn-danger:hover{background:#c5221f}
.btn-sm{padding:5px 12px;font-size:0.85em}
.projects{display:grid;gap:20px}
.project-card{background:#fff;border-radius:12px;padding:20px;box-shadow:0 2px 8px rgba(0,0,0,0.08);border-left:5px solid #1a73e8}
.project-card.high{border-left-color:#ea4335}
.project-card.low{border-left-color:#34a853}
.project-header{display:flex;justify-content:space-between;align-items:center;margin-bottom:10px}
.project-name{font-size:1.3em;font-weight:600}
.priority-badge{padding:3px 10px;border-radius:12px;font-size:0.8em;font-weight:600;text-transform:uppercase}
.priority-badge.high{background:#fce8e6;color:#ea4335}
.priority-badge.med{background:#fef7e0;color:#f9ab00}
.priority-badge.low{background:#e6f4ea;color:#34a853}
.project-desc{color:#5f6368;margin-bottom:15px;font-size:0.95em}
.progress-bar{height:12px;background:#e0e0e0;border-radius:6px;overflow:hidden;margin-bottom:5px}
.progress-fill{height:100%;border-radius:6px;transition:width 0.5s}
.progress-text{font-size:0.85em;color:#5f6368;margin-bottom:15px}
.tasks-section h3{font-size:1em;color:#1a73e8;margin-bottom:10px}
.task-item{display:flex;align-items:center;gap:10px;padding:8px;border-radius:6px;margin-bottom:5px;background:#f8f9fa}
.task-item:hover{background:#e8f0fe}
.task-item.done{opacity:0.6}
.task-item.done .task-name{text-decoration:line-through}
.task-name{flex:1}
.task-check{width:20px;height:20px;cursor:pointer}
.add-task-row{display:flex;gap:8px;margin-top:10px}
.add-task-row input{flex:1;padding:8px;border:1px solid #e0e0e0;border-radius:6px}
.empty{text-align:center;padding:40px;color:#5f6368}
</style></head><body>
<div class="container">
<h1>Project Tracker</h1>
<p class="subtitle">Manage projects and track progress</p>
<div class="add-project">
<h2>New Project</h2>
<div class="form-row">
<input type="text" id="proj-name" placeholder="Project name">
<select id="proj-priority"><option value="high">High</option><option value="med" selected>Medium</option><option value="low">Low</option></select>
<button class="btn" onclick="addProject()">Create Project</button>
</div>
<div class="form-row">
<textarea id="proj-desc" placeholder="Description (optional)"></textarea>
</div>
</div>
<div class="projects">"##);

    if proj_count == 0 {
        p = buf_write(p, r##"<div class="empty">No projects yet. Create one above!</div>"##);
    }

    // Render projects (newest first)
    let mut pi = proj_count;
    while pi > 0 {
        pi -= 1;
        let name = kv_read(make_proj_key(pi, "name")).unwrap_or("");
        if name.is_empty() || name == "(deleted)" { continue; }
        let desc = kv_read(make_proj_key(pi, "desc")).unwrap_or("");
        let priority = kv_read(make_proj_key(pi, "priority")).unwrap_or("med");
        let tasks_str = kv_read(make_proj_key(pi, "tasks")).unwrap_or("");
        let (total, done) = count_tasks(tasks_str);
        let pct = if total > 0 { (done * 100) / total } else { 0 };
        let bar_color = if pct == 100 { "#34a853" } else if pct > 50 { "#1a73e8" } else if pct > 0 { "#f9ab00" } else { "#e0e0e0" };

        p = buf_write(p, r##"<div class="project-card "##);
        p = buf_write(p, priority);
        p = buf_write(p, r##""><div class="project-header"><span class="project-name">"##);
        p = buf_write(p, name);
        p = buf_write(p, r##"</span><div><span class="priority-badge "##);
        p = buf_write(p, priority);
        p = buf_write(p, r##"">"##);
        p = buf_write(p, priority);
        p = buf_write(p, r##"</span> <button class="btn btn-danger btn-sm" onclick="deleteProject("##);
        p = buf_write(p, num_to_str(pi));
        p = buf_write(p, r##")">Delete</button></div></div>"##);
        if !desc.is_empty() {
            p = buf_write(p, r##"<div class="project-desc">"##);
            p = buf_write(p, desc);
            p = buf_write(p, r##"</div>"##);
        }
        p = buf_write(p, r##"<div class="progress-bar"><div class="progress-fill" style="width:"##);
        p = buf_write(p, num_to_str(pct));
        p = buf_write(p, r##"%;background:"##);
        p = buf_write(p, bar_color);
        p = buf_write(p, r##""></div></div><div class="progress-text">"##);
        p = buf_write(p, num_to_str(done));
        p = buf_write(p, "/");
        p = buf_write(p, num_to_str(total));
        p = buf_write(p, " tasks complete (");
        p = buf_write(p, num_to_str(pct));
        p = buf_write(p, r##"%)</div><div class="tasks-section"><h3>Tasks</h3>"##);

        // Render tasks
        if !tasks_str.is_empty() {
            let tb = tasks_str.as_bytes();
            let mut es = 0;
            let mut ei = 0;
            let mut ti: u32 = 0;
            while ei <= tb.len() {
                if ei == tb.len() || tb[ei] == b';' {
                    if ei > es {
                        if let Ok(task_entry) = core::str::from_utf8(&tb[es..ei]) {
                            let teb = task_entry.as_bytes();
                            if let Some(pp) = teb.iter().position(|&b| b == b'|') {
                                let tname = core::str::from_utf8(&teb[..pp]).unwrap_or("?");
                                let tstatus = core::str::from_utf8(&teb[pp+1..]).unwrap_or("todo");
                                let is_done = tstatus == "done";
                                p = buf_write(p, r##"<div class="task-item"##);
                                if is_done { p = buf_write(p, r##" done"##); }
                                p = buf_write(p, r##""><input type="checkbox" class="task-check""##);
                                if is_done { p = buf_write(p, " checked"); }
                                p = buf_write(p, r##" onchange="toggleTask("##);
                                p = buf_write(p, num_to_str(pi));
                                p = buf_write(p, ",");
                                p = buf_write(p, num_to_str(ti));
                                p = buf_write(p, r##")"><span class="task-name">"##);
                                p = buf_write(p, tname);
                                p = buf_write(p, r##"</span></div>"##);
                            }
                            ti += 1;
                        }
                    }
                    es = ei + 1;
                }
                ei += 1;
            }
        }

        p = buf_write(p, r##"<div class="add-task-row"><input type="text" id="task-input-"##);
        p = buf_write(p, num_to_str(pi));
        p = buf_write(p, r##"" placeholder="New task..."><button class="btn btn-sm" onclick="addTask("##);
        p = buf_write(p, num_to_str(pi));
        p = buf_write(p, r##")">Add</button></div></div></div>"##);
    }

    p = buf_write(p, r##"</div></div>
<script>
function addProject(){
  const n=document.getElementById('proj-name').value;
  const pr=document.getElementById('proj-priority').value;
  const d=document.getElementById('proj-desc').value;
  if(!n)return;
  fetch('',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({action:'add_project',name:n,priority:pr,desc:d})}).then(()=>location.reload());
}
function deleteProject(id){
  if(!confirm('Delete this project?'))return;
  fetch('',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({action:'delete_project',id:''+id})}).then(()=>location.reload());
}
function addTask(projId){
  const inp=document.getElementById('task-input-'+projId);
  const t=inp.value;
  if(!t)return;
  fetch('',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({action:'add_task',project:''+projId,task:t})}).then(()=>location.reload());
}
function toggleTask(projId,taskIdx){
  fetch('',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({action:'toggle_task',project:''+projId,task_idx:''+taskIdx})}).then(()=>location.reload());
}
</script></body></html>"##);
    respond(200, buf_as_str(p), "text/html");
}

#[no_mangle]
pub extern "C" fn x402_handle(method_ptr: *const u8, method_len: i32, path_ptr: *const u8, path_len: i32, body_ptr: *const u8, body_len: i32) {
    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };
    let body = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(body_ptr, body_len as usize)) };

    host_log(1, "Project tracker request");

    if method == "POST" {
        let action = find_json_str(body, "action").unwrap_or("");

        if action == "add_project" {
            let name = find_json_str(body, "name").unwrap_or("Project");
            let priority = find_json_str(body, "priority").unwrap_or("med");
            let desc = find_json_str(body, "desc").unwrap_or("");
            let count = parse_u32(kv_read("proj_count").unwrap_or("0"));
            kv_write(make_proj_key(count, "name"), name);
            kv_write(make_proj_key(count, "priority"), priority);
            kv_write(make_proj_key(count, "desc"), desc);
            kv_write(make_proj_key(count, "tasks"), "");
            kv_write("proj_count", num_to_str(count + 1));
            respond(200, r##"{"ok":true}"##, "application/json");
        } else if action == "delete_project" {
            let id = parse_u32(find_json_str(body, "id").unwrap_or("0"));
            kv_write(make_proj_key(id, "name"), "(deleted)");
            respond(200, r##"{"ok":true}"##, "application/json");
        } else if action == "add_task" {
            let proj_id = parse_u32(find_json_str(body, "project").unwrap_or("0"));
            let task = find_json_str(body, "task").unwrap_or("Task");
            let existing = kv_read(make_proj_key(proj_id, "tasks")).unwrap_or("");
            let mut nbuf = [0u8; 4096];
            let mut np = 0;
            if !existing.is_empty() {
                let eb = existing.as_bytes();
                nbuf[..eb.len()].copy_from_slice(eb);
                np = eb.len();
                nbuf[np] = b';'; np += 1;
            }
            let tb = task.as_bytes();
            nbuf[np..np+tb.len()].copy_from_slice(tb);
            np += tb.len();
            nbuf[np] = b'|'; np += 1;
            let todo = b"todo";
            nbuf[np..np+todo.len()].copy_from_slice(todo);
            np += todo.len();
            let nv = unsafe { core::str::from_utf8_unchecked(&nbuf[..np]) };
            kv_write(make_proj_key(proj_id, "tasks"), nv);
            respond(200, r##"{"ok":true}"##, "application/json");
        } else if action == "toggle_task" {
            let proj_id = parse_u32(find_json_str(body, "project").unwrap_or("0"));
            let task_idx = parse_u32(find_json_str(body, "task_idx").unwrap_or("0"));
            let existing = kv_read(make_proj_key(proj_id, "tasks")).unwrap_or("");
            let eb = existing.as_bytes();
            let mut result = [0u8; 4096];
            let mut rp = 0;
            let mut es = 0;
            let mut ei = 0;
            let mut cur_idx: u32 = 0;
            let mut first = true;
            while ei <= eb.len() {
                if ei == eb.len() || eb[ei] == b';' {
                    if ei > es {
                        if !first { result[rp] = b';'; rp += 1; }
                        if cur_idx == task_idx {
                            // Toggle status
                            let segment = &eb[es..ei];
                            if let Some(pp) = segment.iter().position(|&b| b == b'|') {
                                let tname = &segment[..pp];
                                let tstatus = &segment[pp+1..];
                                result[rp..rp+tname.len()].copy_from_slice(tname);
                                rp += tname.len();
                                result[rp] = b'|'; rp += 1;
                                if tstatus == b"done" {
                                    let s = b"todo";
                                    result[rp..rp+s.len()].copy_from_slice(s);
                                    rp += s.len();
                                } else {
                                    let s = b"done";
                                    result[rp..rp+s.len()].copy_from_slice(s);
                                    rp += s.len();
                                }
                            }
                        } else {
                            result[rp..rp+(ei-es)].copy_from_slice(&eb[es..ei]);
                            rp += ei - es;
                        }
                        first = false;
                        cur_idx += 1;
                    }
                    es = ei + 1;
                }
                ei += 1;
            }
            let nv = unsafe { core::str::from_utf8_unchecked(&result[..rp]) };
            kv_write(make_proj_key(proj_id, "tasks"), nv);
            respond(200, r##"{"ok":true}"##, "application/json");
        } else {
            respond(400, r##"{"error":"unknown"}"##, "application/json");
        }
        return;
    }

    render_project_list();
}