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
        if n == 0 { self.push_str("0"); return; }
        let mut digits = [0u8; 10];
        let mut i = 0;
        while n > 0 { digits[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; }
        while i > 0 { i -= 1; unsafe { if self.pos < BUF.len() { BUF[self.pos] = digits[i]; self.pos += 1; } } }
    }

    fn as_str(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&BUF[..self.pos]) }
    }
}

fn parse_num(s: &str) -> u32 {
    let mut n: u32 = 0;
    for &b in s.as_bytes() {
        if b >= b'0' && b <= b'9' { n = n * 10 + (b - b'0') as u32; }
    }
    n
}

#[no_mangle]
pub extern "C" fn handle(method_ptr: *const u8, method_len: i32, _path_ptr: *const u8, _path_len: i32, body_ptr: *const u8, body_len: i32) {
    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };
    let body = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(body_ptr, body_len as usize)) };

    if method.as_bytes()[0] == b'P' {
        // POST: set preferred unit
        if let Some(unit) = find_json_str(body, "unit") {
            kv_write("weather_unit", unit);
            respond(200, "{\"ok\":true}", "application/json");
        } else {
            respond(400, "{\"error\":\"missing unit\"}", "application/json");
        }
        return;
    }

    let unit = kv_read("weather_unit").unwrap_or("C");
    let is_f = unit.as_bytes()[0] == b'F';

    // Track views
    let views = parse_num(kv_read("weather_views").unwrap_or("0"));
    let mut nb = [0u8; 10];
    kv_write("weather_views", num_to_str(&mut nb, views + 1));

    let mut w = BufWriter::new();
    w.push_str("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Weather</title><style>");
    w.push_str("*{margin:0;padding:0;box-sizing:border-box}");
    w.push_str("body{background:linear-gradient(180deg,#0c1445,#1a237e,#283593);color:#e0e0e0;font-family:'Segoe UI',sans-serif;min-height:100vh;padding:40px 20px}");
    w.push_str(".container{max-width:900px;margin:0 auto}");
    w.push_str("h1{text-align:center;font-size:2em;margin-bottom:8px;color:#fff}");
    w.push_str(".subtitle{text-align:center;color:#7986cb;margin-bottom:24px}");
    w.push_str(".unit-toggle{text-align:center;margin-bottom:32px}");
    w.push_str(".toggle-btn{padding:8px 20px;border:2px solid #5c6bc0;background:transparent;color:#9fa8da;border-radius:8px;cursor:pointer;font-size:14px;margin:0 4px;transition:all 0.2s}");
    w.push_str(".toggle-btn.active{background:#5c6bc0;color:#fff}");
    w.push_str(".cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(250px,1fr));gap:20px}");
    w.push_str(".weather-card{background:rgba(255,255,255,0.08);backdrop-filter:blur(10px);border:1px solid rgba(255,255,255,0.1);border-radius:20px;padding:28px;transition:transform 0.2s}");
    w.push_str(".weather-card:hover{transform:translateY(-4px)}");
    w.push_str(".city-name{font-size:22px;font-weight:bold;margin-bottom:4px}");
    w.push_str(".country{color:#7986cb;font-size:14px;margin-bottom:16px}");
    w.push_str(".weather-icon{font-size:56px;margin-bottom:12px}");
    w.push_str(".temp{font-size:48px;font-weight:bold;color:#fff;margin-bottom:4px}");
    w.push_str(".condition{color:#9fa8da;font-size:16px;margin-bottom:16px}");
    w.push_str(".details{display:grid;grid-template-columns:1fr 1fr;gap:8px}");
    w.push_str(".detail{background:rgba(255,255,255,0.05);border-radius:8px;padding:8px 12px}");
    w.push_str(".detail-label{font-size:11px;color:#7986cb;text-transform:uppercase;letter-spacing:1px}");
    w.push_str(".detail-value{font-size:16px;font-weight:600;color:#c5cae9}");
    w.push_str(".footer{text-align:center;margin-top:32px;color:#5c6bc0;font-size:13px}");
    w.push_str("</style></head><body><div class='container'>");
    w.push_str("<h1>Weather Dashboard</h1>");
    w.push_str("<p class='subtitle'>Current conditions around the world</p>");
    w.push_str("<div class='unit-toggle'>");
    w.push_str("<button class='toggle-btn");
    if !is_f { w.push_str(" active"); }
    w.push_str("' onclick='setUnit(\"C\")'>Celsius</button>");
    w.push_str("<button class='toggle-btn");
    if is_f { w.push_str(" active"); }
    w.push_str("' onclick='setUnit(\"F\")'>Fahrenheit</button></div>");

    w.push_str("<div class='cards'>");

    // City data: name, country, icon, temp_c, condition, humidity, wind_kph, feels_like_c
    let cities: [(&str, &str, &str, u32, &str, u32, u32, u32); 5] = [
        ("Tokyo", "Japan", "&#9728;&#65039;", 22, "Sunny", 45, 12, 21),
        ("London", "United Kingdom", "&#9729;&#65039;", 14, "Cloudy", 72, 18, 12),
        ("New York", "United States", "&#127782;&#65039;", 18, "Partly Cloudy", 58, 22, 16),
        ("Sydney", "Australia", "&#9728;&#65039;", 26, "Clear Sky", 38, 15, 25),
        ("Paris", "France", "&#127783;&#65039;", 11, "Light Rain", 85, 24, 8),
    ];

    let mut ci = 0;
    while ci < 5 {
        let (name, country, icon, temp_c, cond, hum, wind, feels_c) = cities[ci];
        let temp_display = if is_f { temp_c * 9 / 5 + 32 } else { temp_c };
        let feels_display = if is_f { feels_c * 9 / 5 + 32 } else { feels_c };
        let unit_sym = if is_f { "F" } else { "C" };

        w.push_str("<div class='weather-card'>");
        w.push_str("<div class='city-name'>");
        w.push_str(name);
        w.push_str("</div><div class='country'>");
        w.push_str(country);
        w.push_str("</div><div class='weather-icon'>");
        w.push_str(icon);
        w.push_str("</div><div class='temp'>");
        w.push_num(temp_display);
        w.push_str("&deg;");
        w.push_str(unit_sym);
        w.push_str("</div><div class='condition'>");
        w.push_str(cond);
        w.push_str("</div><div class='details'>");
        w.push_str("<div class='detail'><div class='detail-label'>Humidity</div><div class='detail-value'>");
        w.push_num(hum);
        w.push_str("%</div></div>");
        w.push_str("<div class='detail'><div class='detail-label'>Wind</div><div class='detail-value'>");
        w.push_num(wind);
        w.push_str(" km/h</div></div>");
        w.push_str("<div class='detail'><div class='detail-label'>Feels Like</div><div class='detail-value'>");
        w.push_num(feels_display);
        w.push_str("&deg;");
        w.push_str(unit_sym);
        w.push_str("</div></div>");
        w.push_str("<div class='detail'><div class='detail-label'>UV Index</div><div class='detail-value'>");
        w.push_num(3 + ci as u32);
        w.push_str("</div></div></div></div>");
        ci += 1;
    }

    w.push_str("</div>");
    w.push_str("<div class='footer'>");
    w.push_num(views + 1);
    w.push_str(" page views</div></div>");

    w.push_str("<script>");
    w.push_str("const BASE=location.pathname;");
    w.push_str("async function setUnit(u){await fetch(BASE,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({unit:u})});location.reload();}");
    w.push_str("</script></body></html>");

    respond(200, w.as_str(), "text/html");
}

fn num_to_str<'a>(buf: &'a mut [u8; 10], mut n: u32) -> &'a str {
    if n == 0 { buf[0] = b'0'; return unsafe { core::str::from_utf8_unchecked(&buf[..1]) }; }
    let mut pos = 0;
    while n > 0 { buf[pos] = b'0' + (n % 10) as u8; n /= 10; pos += 1; }
    buf[..pos].reverse();
    unsafe { core::str::from_utf8_unchecked(&buf[..pos]) }
}
