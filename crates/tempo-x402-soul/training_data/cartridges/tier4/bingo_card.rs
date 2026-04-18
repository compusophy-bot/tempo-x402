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
    while i + kb.len() + 3 < jb.len() {
        if jb[i] == b'"' { let s = i + 1;
            if s + kb.len() < jb.len() && &jb[s..s + kb.len()] == kb && jb[s + kb.len()] == b'"' {
                let mut j = s + kb.len() + 1; while j < jb.len() && (jb[j] == b':' || jb[j] == b' ') { j += 1; }
                if j < jb.len() && jb[j] == b'"' { let vs = j + 1; let mut ve = vs; while ve < jb.len() && jb[ve] != b'"' { ve += 1; } return core::str::from_utf8(&jb[vs..ve]).ok(); }
            }
        } i += 1;
    } None
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

    host_log(0, "bingo_card: handling request");

    if method == "POST" {
        if let Some(action) = find_json_str(body, "action") {
            if action == "mark" {
                if let Some(idx_s) = find_json_str(body, "index") {
                    let idx = parse_u32(idx_s) as usize;
                    let marked = kv_read("bingo_marked").unwrap_or("0000000000000000000000000");
                    let mut mb = [0u8; 25];
                    let src = marked.as_bytes();
                    let mut i = 0;
                    while i < 25 { mb[i] = if i < src.len() { src[i] } else { b'0' }; i += 1; }
                    if idx < 25 { mb[idx] = if mb[idx] == b'1' { b'0' } else { b'1' }; }
                    // Center is always free
                    mb[12] = b'1';
                    let ms = unsafe { core::str::from_utf8_unchecked(&mb) };
                    kv_write("bingo_marked", ms);
                    respond(200, r#"{"ok":true}"#, "application/json");
                } else { respond(400, r#"{"error":"missing index"}"#, "application/json"); }
            } else if action == "new_card" {
                kv_write("bingo_marked", "0000000000001000000000000");
                // Store a seed for card generation
                let seed = find_json_str(body, "seed").unwrap_or("42");
                kv_write("bingo_seed", seed);
                respond(200, r#"{"ok":true}"#, "application/json");
            } else { respond(400, r#"{"error":"unknown action"}"#, "application/json"); }
        } else { respond(400, r#"{"error":"missing action"}"#, "application/json"); }
        return;
    }

    // GET — render bingo card (numbers generated client-side from seed)
    let marked = kv_read("bingo_marked").unwrap_or("0000000000001000000000000");
    let seed = kv_read("bingo_seed").unwrap_or("42");

    let mut w = W::new();
    w.s("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Bingo!</title><style>");
    w.s("*{margin:0;padding:0;box-sizing:border-box}body{background:#1a0a2e;color:#e0e0e0;font-family:'Segoe UI',sans-serif;padding:30px 20px;display:flex;justify-content:center}");
    w.s(".c{max-width:500px;width:100%;text-align:center}h1{color:#ffd700;margin-bottom:8px;font-size:2.5em;text-shadow:0 0 20px rgba(255,215,0,0.5)}");
    w.s(".header{display:grid;grid-template-columns:repeat(5,1fr);gap:4px;margin-bottom:4px}.header span{padding:12px;background:#2d1b69;color:#ffd700;font-weight:bold;font-size:24px;border-radius:6px}");
    w.s(".grid{display:grid;grid-template-columns:repeat(5,1fr);gap:4px}");
    w.s(".cell{aspect-ratio:1;display:flex;align-items:center;justify-content:center;background:#16213e;border-radius:8px;font-size:22px;cursor:pointer;font-weight:bold;transition:all 0.2s;border:2px solid transparent}");
    w.s(".cell:hover{border-color:#7c4dff}.cell.marked{background:#e94560;color:#fff;transform:scale(0.95)}.cell.free{background:#ffd700;color:#000}");
    w.s(".win{font-size:24px;color:#ffd700;margin-top:16px;display:none}.controls{margin:16px 0}");
    w.s("button{padding:12px 28px;background:#7c4dff;color:#fff;border:none;border-radius:8px;cursor:pointer;font-size:16px;margin:4px}button:hover{background:#651fff}");
    w.s("</style></head><body><div class='c'><h1>BINGO!</h1>");
    w.s("<div class='controls'><button onclick='newCard()'>New Card</button></div>");
    w.s("<div class='header'><span>B</span><span>I</span><span>N</span><span>G</span><span>O</span></div>");
    w.s("<div class='grid' id='grid'></div>");
    w.s("<div class='win' id='win'>BINGO! YOU WIN!</div>");
    w.s("</div><script>");
    w.s("const marked='"); w.s(marked); w.s("';const seed="); w.s(seed); w.s(";");
    w.s("function lcg(s){return(s*1103515245+12345)&0x7fffffff;}");
    w.s("function genCard(s){const cols=[[],[],[],[],[]];const ranges=[[1,15],[16,30],[31,45],[46,60],[61,75]];");
    w.s("let st=s;for(let c=0;c<5;c++){const used=new Set();while(cols[c].length<5){st=lcg(st);const v=ranges[c][0]+(st%(ranges[c][1]-ranges[c][0]+1));if(!used.has(v)){used.add(v);cols[c].push(v);}}};return cols;}");
    w.s("const cols=genCard(seed);const grid=document.getElementById('grid');");
    w.s("for(let r=0;r<5;r++){for(let c=0;c<5;c++){const idx=r*5+c;const cell=document.createElement('div');cell.className='cell';");
    w.s("if(idx===12){cell.className='cell free marked';cell.textContent='FREE';}else{cell.textContent=cols[c][r];if(marked[idx]==='1')cell.classList.add('marked');}");
    w.s("cell.onclick=()=>mark(idx);grid.appendChild(cell);}}");
    w.s("checkWin();");
    w.s("async function mark(i){if(i===12)return;await fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'mark',index:String(i)})});location.reload();}");
    w.s("async function newCard(){const s=Math.floor(Math.random()*100000);await fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'new_card',seed:String(s)})});location.reload();}");
    w.s("function checkWin(){const m=marked.split('').map(Number);const wins=[[0,1,2,3,4],[5,6,7,8,9],[10,11,12,13,14],[15,16,17,18,19],[20,21,22,23,24],[0,5,10,15,20],[1,6,11,16,21],[2,7,12,17,22],[3,8,13,18,23],[4,9,14,19,24],[0,6,12,18,24],[4,8,12,16,20]];");
    w.s("for(const w of wins){if(w.every(i=>m[i]))document.getElementById('win').style.display='block';}}");
    w.s("</script></body></html>");
    respond(200, w.out(), "text/html");
}
