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

    host_log(0, "tic_tac_toe: handling request");

    if method == "POST" {
        if let Some(action) = find_json_str(body, "action") {
            if action == "move" {
                if let Some(pos_s) = find_json_str(body, "pos") {
                    let pos = parse_u32(pos_s) as usize;
                    let board_str = kv_read("ttt_board").unwrap_or("_________");
                    let turn_str = kv_read("ttt_turn").unwrap_or("X");
                    let mut board = [0u8; 9];
                    let bs = board_str.as_bytes();
                    let mut i = 0;
                    while i < 9 { board[i] = if i < bs.len() { bs[i] } else { b'_' }; i += 1; }

                    if pos < 9 && board[pos] == b'_' {
                        board[pos] = turn_str.as_bytes()[0];
                        let new_turn = if turn_str == "X" { "O" } else { "X" };
                        let b_str = unsafe { core::str::from_utf8_unchecked(&board) };
                        kv_write("ttt_board", b_str);
                        kv_write("ttt_turn", new_turn);

                        // Check winner
                        let wins: [[usize; 3]; 8] = [
                            [0,1,2],[3,4,5],[6,7,8],[0,3,6],[1,4,7],[2,5,8],[0,4,8],[2,4,6]
                        ];
                        let mut winner = b'_';
                        let mut wi = 0;
                        while wi < 8 {
                            let a = wins[wi][0]; let b = wins[wi][1]; let c = wins[wi][2];
                            if board[a] != b'_' && board[a] == board[b] && board[b] == board[c] {
                                winner = board[a];
                            }
                            wi += 1;
                        }
                        let mut draw = true;
                        i = 0;
                        while i < 9 { if board[i] == b'_' { draw = false; } i += 1; }

                        if winner != b'_' {
                            // Update score
                            if winner == b'X' {
                                let sx = kv_read("ttt_x_wins").map(|s| parse_u32(s)).unwrap_or(0) + 1;
                                let mut w = W::new(); w.n(sx); kv_write("ttt_x_wins", w.out());
                            } else {
                                let so = kv_read("ttt_o_wins").map(|s| parse_u32(s)).unwrap_or(0) + 1;
                                let mut w = W::new(); w.n(so); kv_write("ttt_o_wins", w.out());
                            }
                            kv_write("ttt_status", if winner == b'X' { "X wins!" } else { "O wins!" });
                        } else if draw {
                            let sd = kv_read("ttt_draws").map(|s| parse_u32(s)).unwrap_or(0) + 1;
                            let mut w = W::new(); w.n(sd); kv_write("ttt_draws", w.out());
                            kv_write("ttt_status", "Draw!");
                        } else {
                            kv_write("ttt_status", "playing");
                        }
                    }
                    respond(200, r#"{"ok":true}"#, "application/json");
                } else { respond(400, r#"{"error":"missing pos"}"#, "application/json"); }
            } else if action == "reset" {
                kv_write("ttt_board", "_________");
                kv_write("ttt_turn", "X");
                kv_write("ttt_status", "playing");
                respond(200, r#"{"ok":true}"#, "application/json");
            } else { respond(400, r#"{"error":"unknown"}"#, "application/json"); }
        } else { respond(400, r#"{"error":"missing action"}"#, "application/json"); }
        return;
    }

    // GET — render game
    let board = kv_read("ttt_board").unwrap_or("_________");
    let turn = kv_read("ttt_turn").unwrap_or("X");
    let status = kv_read("ttt_status").unwrap_or("playing");
    let x_wins = kv_read("ttt_x_wins").map(|s| parse_u32(s)).unwrap_or(0);
    let o_wins = kv_read("ttt_o_wins").map(|s| parse_u32(s)).unwrap_or(0);
    let draws = kv_read("ttt_draws").map(|s| parse_u32(s)).unwrap_or(0);

    let mut w = W::new();
    w.s("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Tic Tac Toe</title><style>");
    w.s("*{margin:0;padding:0;box-sizing:border-box}body{background:#0a0a1a;color:#e0e0e0;font-family:'Segoe UI',sans-serif;display:flex;justify-content:center;padding:40px 20px}");
    w.s(".c{text-align:center}h1{color:#7c4dff;margin-bottom:16px;font-size:2em}");
    w.s(".scores{display:flex;gap:20px;justify-content:center;margin-bottom:20px}");
    w.s(".score{background:#16213e;padding:12px 20px;border-radius:8px;min-width:80px}.score .label{font-size:12px;color:#888;text-transform:uppercase}.score .val{font-size:24px;font-weight:bold}");
    w.s(".score.x .val{color:#e94560}.score.o .val{color:#4fc3f7}.score.d .val{color:#888}");
    w.s(".status{font-size:20px;margin-bottom:16px;height:30px}");
    w.s(".grid{display:grid;grid-template-columns:repeat(3,1fr);gap:8px;max-width:300px;margin:0 auto 20px}");
    w.s(".cell{aspect-ratio:1;background:#16213e;border-radius:12px;display:flex;align-items:center;justify-content:center;font-size:48px;font-weight:bold;cursor:pointer;transition:all 0.2s}");
    w.s(".cell:hover{background:#1a2744}.cell.x{color:#e94560}.cell.o{color:#4fc3f7}");
    w.s("button{padding:12px 30px;background:#7c4dff;color:#fff;border:none;border-radius:8px;cursor:pointer;font-size:16px}button:hover{background:#651fff}");
    w.s("</style></head><body><div class='c'><h1>Tic Tac Toe</h1>");
    w.s("<div class='scores'><div class='score x'><div class='label'>X Wins</div><div class='val'>"); w.n(x_wins);
    w.s("</div></div><div class='score d'><div class='label'>Draws</div><div class='val'>"); w.n(draws);
    w.s("</div></div><div class='score o'><div class='label'>O Wins</div><div class='val'>"); w.n(o_wins);
    w.s("</div></div></div>");
    w.s("<div class='status' id='status'>");
    if status == "playing" { w.s(turn); w.s("'s turn"); } else { w.s(status); }
    w.s("</div><div class='grid'>");

    let bb = board.as_bytes();
    let mut i: u32 = 0;
    while i < 9 {
        let ch = if (i as usize) < bb.len() { bb[i as usize] } else { b'_' };
        w.s("<div class='cell");
        if ch == b'X' { w.s(" x"); } else if ch == b'O' { w.s(" o"); }
        w.s("' onclick='play("); w.n(i); w.s(")'>");
        if ch == b'X' { w.s("X"); } else if ch == b'O' { w.s("O"); }
        w.s("</div>");
        i += 1;
    }

    w.s("</div><button onclick='reset()'>New Game</button></div>");
    w.s("<script>const B=location.pathname;const status='"); w.s(status); w.s("';");
    w.s("async function play(i){if(status!=='playing')return;await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'move',pos:String(i)})});location.reload();}");
    w.s("async function reset(){await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'reset'})});location.reload();}");
    w.s("</script></body></html>");
    respond(200, w.out(), "text/html");
}
