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
    while i < b.len() {
        if b[i] >= b'0' && b[i] <= b'9' { n = n * 10 + (b[i] - b'0') as usize; }
        i += 1;
    }
    n
}

static mut TMP: [u8; 32768] = [0u8; 32768];
fn tmp_write(pos: usize, s: &str) -> usize {
    let b = s.as_bytes();
    let end = (pos + b.len()).min(unsafe { TMP.len() });
    unsafe { TMP[pos..end].copy_from_slice(&b[..end - pos]); }
    end
}
fn tmp_as_str(len: usize) -> &'static str {
    unsafe { core::str::from_utf8_unchecked(&TMP[..len]) }
}

// KV "recipes": "title|ingredients|steps\n" per recipe
// ingredients separated by ";", steps separated by ";"
// KV "recipe_count": total count string

fn bytes_contains_ci(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.len() == 0 { return true; }
    if haystack.len() < needle.len() { return false; }
    let mut i = 0;
    while i + needle.len() <= haystack.len() {
        let mut ok = true;
        let mut j = 0;
        while j < needle.len() {
            let a = if haystack[i+j] >= b'A' && haystack[i+j] <= b'Z' { haystack[i+j] + 32 } else { haystack[i+j] };
            let b = if needle[j] >= b'A' && needle[j] <= b'Z' { needle[j] + 32 } else { needle[j] };
            if a != b { ok = false; break; }
            j += 1;
        }
        if ok { return true; }
        i += 1;
    }
    false
}

#[no_mangle]
pub extern "C" fn handle_request(method_ptr: *const u8, method_len: i32, path_ptr: *const u8, path_len: i32, body_ptr: *const u8, body_len: i32) {
    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };
    let path = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(path_ptr, path_len as usize)) };
    let body = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(body_ptr, body_len as usize)) };

    if method == "POST" {
        let action = find_json_str(body, "action").unwrap_or("");
        if action == "add" {
            let title = find_json_str(body, "title").unwrap_or("");
            let ingredients = find_json_str(body, "ingredients").unwrap_or("");
            let steps = find_json_str(body, "steps").unwrap_or("");
            if title.len() > 0 {
                let existing = kv_read("recipes").unwrap_or("");
                let mut tp = 0usize;
                tp = tmp_write(tp, existing);
                tp = tmp_write(tp, title);
                tp = tmp_write(tp, "|");
                tp = tmp_write(tp, ingredients);
                tp = tmp_write(tp, "|");
                tp = tmp_write(tp, steps);
                tp = tmp_write(tp, "\n");
                kv_write("recipes", tmp_as_str(tp));
            }
            respond(200, "{\"ok\":true}", "application/json");
            return;
        }
        if action == "delete" {
            let idx = parse_usize(find_json_str(body, "index").unwrap_or("0"));
            let existing = kv_read("recipes").unwrap_or("");
            let eb = existing.as_bytes();
            let mut tp = 0usize;
            let mut pos = 0usize;
            let mut line_num = 0usize;
            while pos < eb.len() {
                let start = pos;
                while pos < eb.len() && eb[pos] != b'\n' { pos += 1; }
                let line = &eb[start..pos];
                if pos < eb.len() { pos += 1; }
                if line.len() < 3 { line_num += 1; continue; }
                if line_num != idx {
                    let ls = unsafe { core::str::from_utf8_unchecked(line) };
                    tp = tmp_write(tp, ls);
                    tp = tmp_write(tp, "\n");
                }
                line_num += 1;
            }
            kv_write("recipes", tmp_as_str(tp));
            respond(200, "{\"ok\":true}", "application/json");
            return;
        }
        respond(400, "{\"error\":\"unknown\"}", "application/json");
        return;
    }

    // GET — render. Check for ?search= or ?view= query params via path
    let search_query = find_query_param(path, "search");
    let view_idx = find_query_param(path, "view");

    let recipes = kv_read("recipes").unwrap_or("");
    let mut p = 0usize;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Recipe Viewer</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{background:#faf3e0;color:#333;font-family:'Georgia','Segoe UI',serif;min-height:100vh;display:flex;flex-direction:column;align-items:center;padding:20px}
h1{color:#c0392b;margin:20px 0;font-size:2.2em}
.container{width:100%;max-width:650px}
.search-bar{display:flex;gap:10px;margin-bottom:20px}
.search-bar input{flex:1;padding:12px 16px;border:2px solid #ddd;border-radius:25px;font-size:1em;font-family:inherit;background:#fff}
.search-bar input:focus{outline:none;border-color:#c0392b}
.search-bar button{padding:12px 24px;background:#c0392b;color:#fff;border:none;border-radius:25px;cursor:pointer;font-family:inherit;font-size:1em}
.search-bar button:hover{background:#e74c3c}
.add-btn{display:block;width:100%;padding:14px;background:#27ae60;color:#fff;border:none;border-radius:12px;font-size:1.1em;cursor:pointer;font-weight:bold;margin-bottom:20px;font-family:inherit}
.add-btn:hover{background:#2ecc71}
.add-form{display:none;background:#fff;border:2px solid #ddd;border-radius:16px;padding:20px;margin-bottom:20px}
.add-form input,.add-form textarea{width:100%;padding:10px;border:1px solid #ddd;border-radius:8px;margin-bottom:10px;font-family:inherit;font-size:0.95em}
.add-form textarea{min-height:60px;resize:vertical}
.add-form button{padding:10px 20px;background:#c0392b;color:#fff;border:none;border-radius:8px;cursor:pointer;font-family:inherit}
.add-form .hint{font-size:0.8em;color:#999;margin-bottom:10px;font-style:italic}
.recipe-card{background:#fff;border-radius:16px;padding:20px;margin-bottom:15px;box-shadow:0 2px 8px rgba(0,0,0,0.08);transition:all 0.2s;cursor:pointer;border-left:5px solid #c0392b}
.recipe-card:hover{box-shadow:0 4px 16px rgba(0,0,0,0.15);transform:translateY(-2px)}
.recipe-card h3{color:#c0392b;font-size:1.3em;margin-bottom:6px}
.recipe-card .preview{color:#888;font-size:0.9em}
.recipe-detail{background:#fff;border-radius:16px;padding:25px;margin-bottom:20px;box-shadow:0 2px 12px rgba(0,0,0,0.1)}
.recipe-detail h2{color:#c0392b;font-size:1.6em;margin-bottom:15px;border-bottom:2px solid #fae1dd;padding-bottom:10px}
.recipe-detail h3{color:#555;font-size:1.1em;margin:15px 0 8px}
.ingredient-list{list-style:none;padding:0}
.ingredient-list li{padding:6px 0;border-bottom:1px dashed #eee;font-size:0.95em}
.ingredient-list li:before{content:"\\2022 ";color:#c0392b;font-weight:bold}
.steps-list{list-style:none;padding:0;counter-reset:step}
.steps-list li{padding:10px 0 10px 40px;border-bottom:1px solid #eee;position:relative;font-size:0.95em;counter-increment:step}
.steps-list li:before{content:counter(step);position:absolute;left:0;top:8px;width:28px;height:28px;background:#c0392b;color:#fff;border-radius:50%;display:flex;align-items:center;justify-content:center;font-size:0.85em;font-weight:bold}
.back-btn{padding:10px 20px;background:#eee;color:#555;border:none;border-radius:8px;cursor:pointer;margin-bottom:15px;font-family:inherit}
.back-btn:hover{background:#ddd}
.del-btn{float:right;background:none;color:#c0392b;border:none;cursor:pointer;font-size:0.9em;font-family:inherit}
.del-btn:hover{text-decoration:underline}
.empty{text-align:center;color:#999;padding:40px;font-size:1.1em;font-style:italic}
.count{color:#999;margin-bottom:15px;font-size:0.9em}
</style></head><body>
<h1>&#127859; Recipe Viewer</h1>
<div class="container">
"##);

    let rb = recipes.as_bytes();

    // Check if viewing a specific recipe
    if view_idx.len() > 0 {
        let vidx = parse_usize(view_idx);
        let mut pos = 0usize;
        let mut line_num = 0usize;
        while pos < rb.len() {
            let start = pos;
            while pos < rb.len() && rb[pos] != b'\n' { pos += 1; }
            let line = &rb[start..pos];
            if pos < rb.len() { pos += 1; }
            if line.len() < 3 { line_num += 1; continue; }
            if line_num == vidx {
                let mut seps: [usize; 2] = [0; 2];
                let mut sc = 0;
                let mut si = 0;
                while si < line.len() && sc < 2 { if line[si] == b'|' { seps[sc] = si; sc += 1; } si += 1; }
                if sc >= 2 {
                    let title = unsafe { core::str::from_utf8_unchecked(&line[..seps[0]]) };
                    let ingredients = unsafe { core::str::from_utf8_unchecked(&line[seps[0]+1..seps[1]]) };
                    let steps = unsafe { core::str::from_utf8_unchecked(&line[seps[1]+1..]) };

                    p = buf_write(p, r##"<button class="back-btn" onclick="window.location=location.pathname">&#8592; Back to recipes</button>"##);
                    p = buf_write(p, r##"<div class="recipe-detail"><h2>"##);
                    p = buf_write(p, title);
                    p = buf_write(p, r##"<button class="del-btn" onclick="delRecipe("##);
                    p = write_usize(p, vidx);
                    p = buf_write(p, r##")">Delete recipe</button></h2><h3>Ingredients</h3><ul class="ingredient-list">"##);

                    // Split ingredients by ";"
                    let ib = ingredients.as_bytes();
                    let mut ipos = 0usize;
                    while ipos < ib.len() {
                        let istart = ipos;
                        while ipos < ib.len() && ib[ipos] != b';' { ipos += 1; }
                        if ipos > istart {
                            let ing = unsafe { core::str::from_utf8_unchecked(&ib[istart..ipos]) };
                            p = buf_write(p, "<li>");
                            p = buf_write(p, ing);
                            p = buf_write(p, "</li>");
                        }
                        if ipos < ib.len() { ipos += 1; }
                    }

                    p = buf_write(p, r##"</ul><h3>Steps</h3><ol class="steps-list">"##);

                    // Split steps by ";"
                    let sb = steps.as_bytes();
                    let mut spos = 0usize;
                    while spos < sb.len() {
                        let sstart = spos;
                        while spos < sb.len() && sb[spos] != b';' { spos += 1; }
                        if spos > sstart {
                            let step = unsafe { core::str::from_utf8_unchecked(&sb[sstart..spos]) };
                            p = buf_write(p, "<li>");
                            p = buf_write(p, step);
                            p = buf_write(p, "</li>");
                        }
                        if spos < sb.len() { spos += 1; }
                    }

                    p = buf_write(p, "</ol></div>");
                }
                break;
            }
            line_num += 1;
        }
    } else {
        // List view
        p = buf_write(p, r##"<div class="search-bar"><input type="text" id="searchInp" placeholder="Search recipes..." value=""##);
        p = buf_write(p, search_query);
        p = buf_write(p, r##""><button onclick="doSearch()">Search</button></div>
<button class="add-btn" onclick="toggleForm()">+ Add New Recipe</button>
<div class="add-form" id="addForm">
<input type="text" id="rTitle" placeholder="Recipe name">
<div class="hint">Ingredients (separate with semicolons)</div>
<textarea id="rIngredients" placeholder="2 cups flour; 1 egg; 1 cup milk; pinch of salt"></textarea>
<div class="hint">Steps (separate with semicolons)</div>
<textarea id="rSteps" placeholder="Mix dry ingredients; Add wet ingredients; Stir until smooth; Cook on medium heat"></textarea>
<button onclick="addRecipe()">Save Recipe</button>
</div>
"##);

        let mut pos = 0usize;
        let mut idx = 0usize;
        let mut shown = 0usize;
        while pos < rb.len() {
            let start = pos;
            while pos < rb.len() && rb[pos] != b'\n' { pos += 1; }
            let line = &rb[start..pos];
            if pos < rb.len() { pos += 1; }
            if line.len() < 3 { idx += 1; continue; }

            // Filter by search
            if search_query.len() > 0 {
                if !bytes_contains_ci(line, search_query.as_bytes()) {
                    idx += 1;
                    continue;
                }
            }

            let mut seps: [usize; 2] = [0; 2];
            let mut sc = 0;
            let mut si = 0;
            while si < line.len() && sc < 2 { if line[si] == b'|' { seps[sc] = si; sc += 1; } si += 1; }
            if sc >= 2 {
                let title = unsafe { core::str::from_utf8_unchecked(&line[..seps[0]]) };
                let ingredients = unsafe { core::str::from_utf8_unchecked(&line[seps[0]+1..seps[1]]) };
                // Show first 60 chars of ingredients as preview
                let preview_end = if ingredients.len() > 60 { 60 } else { ingredients.len() };
                let preview = unsafe { core::str::from_utf8_unchecked(&line[seps[0]+1..seps[0]+1+preview_end]) };

                p = buf_write(p, r##"<div class="recipe-card" onclick="viewRecipe("##);
                p = write_usize(p, idx);
                p = buf_write(p, r##")"><h3>"##);
                p = buf_write(p, title);
                p = buf_write(p, r##"</h3><div class="preview">"##);
                p = buf_write(p, preview);
                if ingredients.len() > 60 { p = buf_write(p, "..."); }
                p = buf_write(p, "</div></div>");
                shown += 1;
            }
            idx += 1;
        }

        if shown == 0 {
            if search_query.len() > 0 {
                p = buf_write(p, r##"<div class="empty">No recipes match your search.</div>"##);
            } else {
                p = buf_write(p, r##"<div class="empty">No recipes yet. Add your first recipe!</div>"##);
            }
        }
    }

    p = buf_write(p, r##"</div>
<script>
var B=location.pathname;
function toggleForm(){var f=document.getElementById('addForm');f.style.display=f.style.display==='block'?'none':'block'}
function addRecipe(){var t=document.getElementById('rTitle').value.trim();var i=document.getElementById('rIngredients').value.trim();var s=document.getElementById('rSteps').value.trim();if(!t)return alert('Enter recipe name');fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'add',title:t,ingredients:i,steps:s})}).then(()=>location.reload())}
function viewRecipe(i){window.location=B+'?view='+i}
function doSearch(){var q=document.getElementById('searchInp').value.trim();if(q)window.location=B+'?search='+encodeURIComponent(q);else window.location=B}
function delRecipe(i){if(!confirm('Delete this recipe?'))return;fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'delete',index:String(i)})}).then(()=>window.location=B)}
document.getElementById('searchInp')&&document.getElementById('searchInp').addEventListener('keydown',function(e){if(e.key==='Enter')doSearch()});
</script></body></html>"##);

    respond(200, buf_as_str(p), "text/html");
}

// Extract a query parameter from path like "/c/slug?key=value"
fn find_query_param<'a>(path: &'a str, key: &str) -> &'a str {
    let pb = path.as_bytes();
    let kb = key.as_bytes();
    // Find '?'
    let mut qi = 0;
    while qi < pb.len() && pb[qi] != b'?' { qi += 1; }
    if qi >= pb.len() { return ""; }
    qi += 1; // skip '?'
    // Search for key=
    while qi < pb.len() {
        let start = qi;
        // Check if this param starts with key
        if qi + kb.len() + 1 <= pb.len() {
            let mut match_ok = true;
            let mut ki = 0;
            while ki < kb.len() {
                if pb[qi + ki] != kb[ki] { match_ok = false; break; }
                ki += 1;
            }
            if match_ok && qi + kb.len() < pb.len() && pb[qi + kb.len()] == b'=' {
                let val_start = qi + kb.len() + 1;
                let mut val_end = val_start;
                while val_end < pb.len() && pb[val_end] != b'&' { val_end += 1; }
                return unsafe { core::str::from_utf8_unchecked(&pb[val_start..val_end]) };
            }
        }
        // Skip to next param
        while qi < pb.len() && pb[qi] != b'&' { qi += 1; }
        if qi < pb.len() { qi += 1; }
    }
    ""
}
