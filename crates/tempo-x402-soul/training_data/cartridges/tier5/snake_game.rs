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

struct BufWriter { pos: usize }
impl BufWriter {
    fn new() -> Self { Self { pos: 0 } }
    fn push_str(&mut self, s: &str) {
        let bytes = s.as_bytes();
        unsafe {
            let end = (self.pos + bytes.len()).min(BUF.len());
            BUF[self.pos..end].copy_from_slice(&bytes[..end - self.pos]);
            self.pos = end;
        }
    }
    fn push_num(&mut self, mut n: u32) {
        if n == 0 { self.push_str("0"); return; }
        let mut d = [0u8; 10];
        let mut i = 0;
        while n > 0 { d[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; }
        while i > 0 { i -= 1; unsafe { if self.pos < BUF.len() { BUF[self.pos] = d[i]; self.pos += 1; } } }
    }
    fn as_str(&self) -> &str { unsafe { core::str::from_utf8_unchecked(&BUF[..self.pos]) } }
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}

fn parse_u32(s: &str) -> u32 {
    let b = s.as_bytes();
    let mut r: u32 = 0;
    let mut i = 0;
    while i < b.len() {
        if b[i] >= b'0' && b[i] <= b'9' {
            r = r.wrapping_mul(10).wrapping_add((b[i] - b'0') as u32);
        }
        i += 1;
    }
    r
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe {
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(request_ptr, request_len as usize))
    };
    let method = find_json_str(request, "method").unwrap_or("GET");
    let path = find_json_str(request, "path").unwrap_or("/");
    let body = find_json_str(request, "body").unwrap_or("");

    host_log(0, "snake_game: handling request");

    // POST /score — submit high score
    if method == "POST" && path == "/score" {
        let score_str = find_json_str(body, "score").unwrap_or("0");
        let name = find_json_str(body, "name").unwrap_or("anon");
        let score = parse_u32(score_str);

        let current_high = match kv_read("snake_high") {
            Some(s) => parse_u32(s),
            None => 0,
        };

        if score > current_high {
            // Store new high score in scratch
            let mut num_buf = [0u8; 12];
            let mut np = 0;
            if score == 0 { num_buf[0] = b'0'; np = 1; }
            else {
                let mut d = [0u8; 10]; let mut di = 0; let mut n = score;
                while n > 0 { d[di] = b'0' + (n % 10) as u8; n /= 10; di += 1; }
                while di > 0 { di -= 1; num_buf[np] = d[di]; np += 1; }
            }
            let s = unsafe { core::str::from_utf8_unchecked(&num_buf[..np]) };
            kv_write("snake_high", s);
            kv_write("snake_champion", name);

            // Track top 5 scores
            let count = match kv_read("snake_score_count") {
                Some(s) => parse_u32(s),
                None => 0,
            };
            let new_count = if count < 5 { count + 1 } else { 5 };
            // Shift scores down to make room at position 0
            let mut si = if count < 4 { count } else { 4 };
            while si > 0 {
                let mut src_key = [0u8; 24];
                let mut sk = 0;
                let pfix = b"snake_s_";
                let mut pi = 0;
                while pi < pfix.len() { src_key[sk] = pfix[pi]; sk += 1; pi += 1; }
                src_key[sk] = b'0' + (si - 1) as u8; sk += 1;
                let src = unsafe { core::str::from_utf8_unchecked(&src_key[..sk]) };

                let mut dst_key = [0u8; 24];
                let mut dk = 0;
                pi = 0;
                while pi < pfix.len() { dst_key[dk] = pfix[pi]; dk += 1; pi += 1; }
                dst_key[dk] = b'0' + si as u8; dk += 1;
                let dst = unsafe { core::str::from_utf8_unchecked(&dst_key[..dk]) };

                if let Some(val) = kv_read(src) {
                    kv_write(dst, val);
                }
                si -= 1;
            }
            // Write new entry at position 0
            let mut entry_buf = [0u8; 128];
            let mut ep = 0;
            let nb = name.as_bytes();
            let mut i = 0;
            while i < nb.len() && ep < 120 { entry_buf[ep] = nb[i]; ep += 1; i += 1; }
            entry_buf[ep] = b'|'; ep += 1;
            i = 0;
            while i < np && ep < 127 { entry_buf[ep] = num_buf[i]; ep += 1; i += 1; }
            let entry = unsafe { core::str::from_utf8_unchecked(&entry_buf[..ep]) };
            kv_write("snake_s_0", entry);

            let mut nc_buf = [0u8; 4];
            nc_buf[0] = b'0' + new_count as u8;
            let nc = unsafe { core::str::from_utf8_unchecked(&nc_buf[..1]) };
            kv_write("snake_score_count", nc);

            respond(200, r#"{"new_high":true}"#, "application/json");
        } else {
            respond(200, r#"{"new_high":false}"#, "application/json");
        }
        return;
    }

    // GET /scores — leaderboard JSON
    if path == "/scores" {
        let mut w = BufWriter::new();
        w.push_str(r#"{"high":"#);
        let high = match kv_read("snake_high") { Some(s) => parse_u32(s), None => 0 };
        w.push_num(high);
        w.push_str(r#","champion":""#);
        w.push_str(kv_read("snake_champion").unwrap_or("none"));
        w.push_str(r#"","scores":["#);

        let count = match kv_read("snake_score_count") { Some(s) => parse_u32(s), None => 0 };
        let mut idx = 0u32;
        while idx < count && idx < 5 {
            if idx > 0 { w.push_str(","); }
            let mut key = [0u8; 24];
            let mut kp = 0;
            let pfix = b"snake_s_";
            let mut pi = 0;
            while pi < pfix.len() { key[kp] = pfix[pi]; kp += 1; pi += 1; }
            key[kp] = b'0' + idx as u8; kp += 1;
            let k = unsafe { core::str::from_utf8_unchecked(&key[..kp]) };
            if let Some(entry) = kv_read(k) {
                let eb = entry.as_bytes();
                let mut sp = 0;
                while sp < eb.len() && eb[sp] != b'|' { sp += 1; }
                let nm = unsafe { core::str::from_utf8_unchecked(&eb[..sp]) };
                let sc = if sp + 1 < eb.len() { unsafe { core::str::from_utf8_unchecked(&eb[sp+1..]) } } else { "0" };
                w.push_str(r#"{"name":""#);
                w.push_str(nm);
                w.push_str(r#"","score":"#);
                w.push_str(sc);
                w.push_str("}");
            }
            idx += 1;
        }
        w.push_str("]}");
        respond(200, w.as_str(), "application/json");
        return;
    }

    // GET / — render game
    let mut w = BufWriter::new();
    w.push_str("<!DOCTYPE html><html><head><meta charset='utf-8'><title>Snake Game</title><style>");
    w.push_str("*{margin:0;padding:0;box-sizing:border-box}");
    w.push_str("body{background:#0a0a1a;color:#e0e0e0;font-family:'Courier New',monospace;display:flex;flex-direction:column;align-items:center;min-height:100vh;padding:20px}");
    w.push_str("h1{color:#4caf50;font-size:2rem;margin-bottom:10px;text-shadow:0 0 20px rgba(76,175,80,0.5)}");
    w.push_str(".info{display:flex;gap:30px;margin-bottom:15px;font-size:1.1rem}");
    w.push_str(".info span{color:#888}.info .val{color:#7c4dff;font-weight:bold}");
    w.push_str("canvas{border:2px solid #333;border-radius:4px;background:#111}");
    w.push_str(".controls{margin-top:15px;display:flex;gap:10px}");
    w.push_str(".controls button{padding:8px 20px;border:1px solid #444;border-radius:6px;background:#1a1a3e;color:#e0e0e0;font-size:1rem;cursor:pointer;transition:all 0.2s}");
    w.push_str(".controls button:hover{background:#2d1b69;border-color:#7c4dff}");
    w.push_str("#gameOver{display:none;position:fixed;top:50%;left:50%;transform:translate(-50%,-50%);background:rgba(10,10,26,0.95);border:2px solid #e94560;border-radius:16px;padding:40px;text-align:center;z-index:10}");
    w.push_str("#gameOver h2{color:#e94560;font-size:1.8rem;margin-bottom:12px}");
    w.push_str("#gameOver .final{font-size:2.5rem;color:#7c4dff;margin:10px 0}");
    w.push_str("#gameOver input{padding:8px 12px;border:1px solid #444;border-radius:6px;background:#1a1a3e;color:#e0e0e0;font-size:1rem;margin:10px 0;text-align:center}");
    w.push_str("#gameOver button{padding:10px 30px;border:none;border-radius:8px;background:#4caf50;color:#fff;font-size:1rem;cursor:pointer;margin-top:10px}");
    w.push_str(".leaderboard{margin-top:20px;background:#111;border:1px solid #333;border-radius:8px;padding:16px;min-width:300px}");
    w.push_str(".leaderboard h3{color:#ffcc00;margin-bottom:10px}.leaderboard .entry{display:flex;justify-content:space-between;padding:6px 0;border-bottom:1px solid #222}");
    w.push_str(".leaderboard .rank{color:#7c4dff;width:30px}.leaderboard .name{flex:1;color:#ccc}.leaderboard .score{color:#4caf50;font-weight:bold}");
    w.push_str("</style></head><body>");

    w.push_str("<h1>SNAKE</h1>");
    w.push_str("<div class='info'><span>Score: <span class='val' id='score'>0</span></span>");
    w.push_str("<span>High: <span class='val' id='highScore'>0</span></span>");
    w.push_str("<span>Speed: <span class='val' id='speed'>1</span></span></div>");
    w.push_str("<canvas id='game' width='400' height='400'></canvas>");
    w.push_str("<div class='controls'><button onclick='startGame()'>New Game</button><button onclick='togglePause()'>Pause</button></div>");

    // Game over modal
    w.push_str("<div id='gameOver'><h2>Game Over!</h2><div class='final' id='finalScore'>0</div>");
    w.push_str("<input id='playerName' placeholder='Your name' maxlength='15' value='anon'>");
    w.push_str("<br><button onclick='submitScore()'>Submit Score</button></div>");

    // Leaderboard
    w.push_str("<div class='leaderboard'><h3>Leaderboard</h3><div id='leaders'>Loading...</div></div>");

    w.push_str("<script>");
    w.push_str("const canvas=document.getElementById('game');const ctx=canvas.getContext('2d');");
    w.push_str("const GRID=20;const CELLS=20;let snake,food,dir,nextDir,score,gameLoop,paused,gameActive;");

    w.push_str("function startGame(){");
    w.push_str("snake=[{x:10,y:10},{x:9,y:10},{x:8,y:10}];");
    w.push_str("dir={x:1,y:0};nextDir={x:1,y:0};score=0;paused=false;gameActive=true;");
    w.push_str("document.getElementById('score').textContent='0';");
    w.push_str("document.getElementById('speed').textContent='1';");
    w.push_str("document.getElementById('gameOver').style.display='none';");
    w.push_str("placeFood();if(gameLoop)clearInterval(gameLoop);gameLoop=setInterval(tick,150);}");

    w.push_str("function placeFood(){let x,y;do{x=Math.floor(Math.random()*CELLS);y=Math.floor(Math.random()*CELLS);}while(snake.some(s=>s.x===x&&s.y===y));food={x,y};}");

    w.push_str("function tick(){if(paused||!gameActive)return;");
    w.push_str("dir=nextDir;const head={x:snake[0].x+dir.x,y:snake[0].y+dir.y};");
    // Wall collision
    w.push_str("if(head.x<0||head.x>=CELLS||head.y<0||head.y>=CELLS){endGame();return;}");
    // Self collision
    w.push_str("if(snake.some(s=>s.x===head.x&&s.y===head.y)){endGame();return;}");
    w.push_str("snake.unshift(head);");
    w.push_str("if(head.x===food.x&&head.y===food.y){score++;document.getElementById('score').textContent=score;");
    w.push_str("const spd=Math.floor(score/5)+1;document.getElementById('speed').textContent=spd;");
    w.push_str("clearInterval(gameLoop);gameLoop=setInterval(tick,Math.max(50,150-score*5));placeFood();}");
    w.push_str("else{snake.pop();}draw();}");

    w.push_str("function draw(){ctx.fillStyle='#111';ctx.fillRect(0,0,400,400);");
    // Grid
    w.push_str("ctx.strokeStyle='#1a1a1a';for(let i=0;i<=CELLS;i++){ctx.beginPath();ctx.moveTo(i*GRID,0);ctx.lineTo(i*GRID,400);ctx.stroke();ctx.beginPath();ctx.moveTo(0,i*GRID);ctx.lineTo(400,i*GRID);ctx.stroke();}");
    // Food
    w.push_str("ctx.fillStyle='#e94560';ctx.shadowColor='#e94560';ctx.shadowBlur=10;ctx.beginPath();ctx.arc(food.x*GRID+GRID/2,food.y*GRID+GRID/2,GRID/2-2,0,Math.PI*2);ctx.fill();ctx.shadowBlur=0;");
    // Snake
    w.push_str("snake.forEach((s,i)=>{const g=Math.max(100,255-i*15);ctx.fillStyle=i===0?'#4caf50':`rgb(0,${g},0)`;ctx.shadowColor='#4caf50';ctx.shadowBlur=i===0?8:0;");
    w.push_str("ctx.fillRect(s.x*GRID+1,s.y*GRID+1,GRID-2,GRID-2);ctx.shadowBlur=0;});");
    // Eyes on head
    w.push_str("const h=snake[0];ctx.fillStyle='#fff';ctx.fillRect(h.x*GRID+4,h.y*GRID+4,4,4);ctx.fillRect(h.x*GRID+12,h.y*GRID+4,4,4);}");

    w.push_str("function endGame(){gameActive=false;clearInterval(gameLoop);");
    w.push_str("document.getElementById('finalScore').textContent=score;");
    w.push_str("document.getElementById('gameOver').style.display='block';}");

    w.push_str("function togglePause(){if(!gameActive)return;paused=!paused;}");

    w.push_str("async function submitScore(){const name=document.getElementById('playerName').value||'anon';");
    w.push_str("try{const r=await fetch(window.location.pathname+'/score',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({name:name,score:String(score)})});");
    w.push_str("const d=await r.json();if(d.new_high){document.getElementById('highScore').textContent=score;}");
    w.push_str("document.getElementById('gameOver').style.display='none';loadLeaderboard();}catch(e){console.error(e);}}");

    w.push_str("async function loadLeaderboard(){try{const r=await fetch(window.location.pathname+'/scores');const d=await r.json();");
    w.push_str("document.getElementById('highScore').textContent=d.high;");
    w.push_str("let html='';if(d.scores&&d.scores.length>0){d.scores.forEach((s,i)=>{");
    w.push_str("html+='<div class=\"entry\"><span class=\"rank\">#'+(i+1)+'</span><span class=\"name\">'+s.name+'</span><span class=\"score\">'+s.score+'</span></div>';});}");
    w.push_str("else{html='<div style=\"color:#555\">No scores yet</div>';}");
    w.push_str("document.getElementById('leaders').innerHTML=html;}catch(e){}}");

    w.push_str("document.addEventListener('keydown',e=>{");
    w.push_str("if(e.key==='ArrowUp'&&dir.y!==1)nextDir={x:0,y:-1};");
    w.push_str("else if(e.key==='ArrowDown'&&dir.y!==-1)nextDir={x:0,y:1};");
    w.push_str("else if(e.key==='ArrowLeft'&&dir.x!==1)nextDir={x:-1,y:0};");
    w.push_str("else if(e.key==='ArrowRight'&&dir.x!==-1)nextDir={x:1,y:0};");
    w.push_str("e.preventDefault();});");

    w.push_str("loadLeaderboard();startGame();");
    w.push_str("</script></body></html>");

    respond(200, w.as_str(), "text/html");
}
