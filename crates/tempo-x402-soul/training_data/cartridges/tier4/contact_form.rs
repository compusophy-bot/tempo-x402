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

struct BufWriter {
    pos: usize,
}

impl BufWriter {
    fn new() -> Self { Self { pos: 0 } }

    fn push_str(&mut self, s: &str) {
        let bytes = s.as_bytes();
        unsafe {
            let end = (self.pos + bytes.len()).min(BUF.len());
            let copy_len = end - self.pos;
            BUF[self.pos..end].copy_from_slice(&bytes[..copy_len]);
            self.pos = end;
        }
    }

    fn push_num(&mut self, mut n: u32) {
        if n == 0 {
            self.push_str("0");
            return;
        }
        let mut digits = [0u8; 10];
        let mut i = 0;
        while n > 0 {
            digits[i] = b'0' + (n % 10) as u8;
            n /= 10;
            i += 1;
        }
        while i > 0 {
            i -= 1;
            unsafe {
                if self.pos < BUF.len() {
                    BUF[self.pos] = digits[i];
                    self.pos += 1;
                }
            }
        }
    }

    fn as_str(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&BUF[..self.pos]) }
    }
}

fn parse_num(s: &str) -> u32 {
    let mut n: u32 = 0;
    for &b in s.as_bytes() {
        if b >= b'0' && b <= b'9' {
            n = n * 10 + (b - b'0') as u32;
        }
    }
    n
}

#[no_mangle]
pub extern "C" fn handle(method_ptr: *const u8, method_len: i32, _path_ptr: *const u8, _path_len: i32, body_ptr: *const u8, body_len: i32) {
    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };
    let body = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(body_ptr, body_len as usize)) };

    if method.as_bytes()[0] == b'P' {
        let name = find_json_str(body, "name").unwrap_or("Anonymous");
        let email = find_json_str(body, "email").unwrap_or("no-email");
        let message = find_json_str(body, "message").unwrap_or("");

        if message.len() == 0 {
            respond(400, "{\"error\":\"message required\"}", "application/json");
            return;
        }

        let count_str = kv_read("contact_count").unwrap_or("0");
        let count = parse_num(count_str);
        let new_count = count + 1;

        // Store the submission
        let mut w = BufWriter::new();
        w.push_str("{\"name\":\"");
        w.push_str(name);
        w.push_str("\",\"email\":\"");
        w.push_str(email);
        w.push_str("\",\"message\":\"");
        w.push_str(message);
        w.push_str("\"}");

        let mut key_buf = [0u8; 32];
        let key = make_key(&mut key_buf, "contact_", new_count);
        kv_write(key, w.as_str());

        // Update count
        let mut num_buf = [0u8; 10];
        let num_str = num_to_str(&mut num_buf, new_count);
        kv_write("contact_count", num_str);

        host_log(1, "New contact form submission");
        respond(200, "{\"ok\":true}", "application/json");
        return;
    }

    // GET: render form
    let count = parse_num(kv_read("contact_count").unwrap_or("0"));
    let mut w = BufWriter::new();
    w.push_str("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Contact Us</title><style>");
    w.push_str("*{margin:0;padding:0;box-sizing:border-box}");
    w.push_str("body{background:linear-gradient(135deg,#0f0c29,#302b63,#24243e);color:#e0e0e0;font-family:'Segoe UI',sans-serif;min-height:100vh;display:flex;justify-content:center;padding:40px 20px}");
    w.push_str(".container{max-width:560px;width:100%}");
    w.push_str("h1{text-align:center;margin-bottom:8px;font-size:2.2em;background:linear-gradient(90deg,#667eea,#764ba2);-webkit-background-clip:text;-webkit-text-fill-color:transparent}");
    w.push_str(".subtitle{text-align:center;color:#8888aa;margin-bottom:32px}");
    w.push_str(".form-group{margin-bottom:20px}");
    w.push_str("label{display:block;margin-bottom:6px;color:#aab;font-size:14px;font-weight:600;text-transform:uppercase;letter-spacing:1px}");
    w.push_str("input,textarea{width:100%;padding:14px 16px;border:2px solid #3a3a5c;background:#1a1a3e;color:#e0e0e0;border-radius:10px;font-size:16px;outline:none;font-family:inherit;transition:border-color 0.3s}");
    w.push_str("input:focus,textarea:focus{border-color:#667eea}");
    w.push_str("textarea{height:140px;resize:vertical}");
    w.push_str(".submit-btn{width:100%;padding:16px;background:linear-gradient(90deg,#667eea,#764ba2);color:#fff;border:none;border-radius:10px;font-size:18px;font-weight:bold;cursor:pointer;transition:opacity 0.2s}");
    w.push_str(".submit-btn:hover{opacity:0.9}");
    w.push_str(".submit-btn:disabled{opacity:0.5;cursor:not-allowed}");
    w.push_str(".success{display:none;text-align:center;padding:40px;background:#1a3a2e;border:2px solid #2ea043;border-radius:12px;margin-top:20px}");
    w.push_str(".success h2{color:#2ea043;margin-bottom:10px}");
    w.push_str(".success.show{display:block}");
    w.push_str(".stats{text-align:center;margin-top:24px;color:#666;font-size:13px}");
    w.push_str(".submissions{margin-top:32px}");
    w.push_str(".sub-card{background:#1a1a3e;border:1px solid #3a3a5c;border-radius:10px;padding:16px;margin-bottom:12px}");
    w.push_str(".sub-card .name{color:#667eea;font-weight:bold;font-size:15px}");
    w.push_str(".sub-card .email{color:#8888aa;font-size:13px}");
    w.push_str(".sub-card .msg{margin-top:8px;color:#ccc;font-size:14px;line-height:1.5}");
    w.push_str("</style></head><body><div class='container'>");
    w.push_str("<h1>Contact Us</h1><p class='subtitle'>We'd love to hear from you</p>");
    w.push_str("<form id='contactForm' onsubmit='return submitForm(event)'>");
    w.push_str("<div class='form-group'><label>Name</label><input type='text' id='name' placeholder='Your name' required></div>");
    w.push_str("<div class='form-group'><label>Email</label><input type='email' id='email' placeholder='your@email.com' required></div>");
    w.push_str("<div class='form-group'><label>Message</label><textarea id='message' placeholder='Tell us what you think...' required></textarea></div>");
    w.push_str("<button type='submit' class='submit-btn' id='submitBtn'>Send Message</button>");
    w.push_str("</form>");
    w.push_str("<div class='success' id='successMsg'><h2>&#10003; Message Sent!</h2><p>Thank you for reaching out. We'll get back to you soon.</p></div>");
    w.push_str("<div class='stats'>");
    w.push_num(count);
    w.push_str(" messages received</div>");

    // Show recent submissions
    if count > 0 {
        w.push_str("<div class='submissions'><h3 style='color:#667eea;margin-bottom:16px'>Recent Messages</h3>");
        let start = if count > 5 { count - 5 } else { 0 };
        let mut i = count;
        while i > start {
            let mut key_buf = [0u8; 32];
            let key = make_key(&mut key_buf, "contact_", i);
            if let Some(data) = kv_read(key) {
                let name = find_json_str(data, "name").unwrap_or("?");
                let email = find_json_str(data, "email").unwrap_or("?");
                let msg = find_json_str(data, "message").unwrap_or("?");
                w.push_str("<div class='sub-card'><div class='name'>");
                w.push_str(name);
                w.push_str("</div><div class='email'>");
                w.push_str(email);
                w.push_str("</div><div class='msg'>");
                w.push_str(msg);
                w.push_str("</div></div>");
            }
            i -= 1;
        }
        w.push_str("</div>");
    }

    w.push_str("</div><script>");
    w.push_str("async function submitForm(e){e.preventDefault();const btn=document.getElementById('submitBtn');btn.disabled=true;btn.textContent='Sending...';");
    w.push_str("const data={name:document.getElementById('name').value,email:document.getElementById('email').value,message:document.getElementById('message').value};");
    w.push_str("try{await fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(data)});");
    w.push_str("document.getElementById('contactForm').style.display='none';document.getElementById('successMsg').classList.add('show');");
    w.push_str("setTimeout(()=>location.reload(),2000);}catch(err){btn.disabled=false;btn.textContent='Send Message';alert('Failed to send');}return false;}");
    w.push_str("</script></body></html>");

    respond(200, w.as_str(), "text/html");
}

fn make_key<'a>(buf: &'a mut [u8; 32], prefix: &str, num: u32) -> &'a str {
    let pb = prefix.as_bytes();
    let mut pos = 0;
    while pos < pb.len() && pos < 32 {
        buf[pos] = pb[pos];
        pos += 1;
    }
    let mut n = num;
    if n == 0 {
        buf[pos] = b'0';
        pos += 1;
    } else {
        let start = pos;
        while n > 0 {
            buf[pos] = b'0' + (n % 10) as u8;
            n /= 10;
            pos += 1;
        }
        buf[start..pos].reverse();
    }
    unsafe { core::str::from_utf8_unchecked(&buf[..pos]) }
}

fn num_to_str<'a>(buf: &'a mut [u8; 10], mut n: u32) -> &'a str {
    if n == 0 {
        buf[0] = b'0';
        return unsafe { core::str::from_utf8_unchecked(&buf[..1]) };
    }
    let mut pos = 0;
    while n > 0 {
        buf[pos] = b'0' + (n % 10) as u8;
        n /= 10;
        pos += 1;
    }
    buf[..pos].reverse();
    unsafe { core::str::from_utf8_unchecked(&buf[..pos]) }
}
