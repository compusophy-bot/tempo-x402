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

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}

const BODY: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Resume — Jordan Blake</title>
<style>
  *, *::before, *::after { margin: 0; padding: 0; box-sizing: border-box; }
  body { font-family: 'Segoe UI', system-ui, sans-serif; background: #f1f5f9; padding: 40px 20px; color: #1e293b; }
  .resume {
    max-width: 820px; margin: 0 auto; background: #fff;
    border-radius: 12px; box-shadow: 0 2px 16px rgba(0,0,0,0.06);
    display: grid; grid-template-columns: 280px 1fr;
    overflow: hidden;
  }
  .sidebar {
    background: #1e293b; color: #e2e8f0; padding: 40px 28px;
  }
  .avatar {
    width: 100px; height: 100px; border-radius: 50%; margin: 0 auto 20px;
    background: linear-gradient(135deg, #38bdf8, #818cf8);
    display: flex; align-items: center; justify-content: center;
    font-size: 36px; font-weight: 800; color: #fff;
    border: 3px solid rgba(255,255,255,0.2);
  }
  .sidebar .name { text-align: center; font-size: 1.3rem; font-weight: 700; margin-bottom: 4px; }
  .sidebar .title { text-align: center; font-size: 0.88rem; color: #94a3b8; margin-bottom: 32px; }
  .sidebar h3 {
    font-size: 0.72rem; text-transform: uppercase; letter-spacing: 2px;
    color: #64748b; margin-bottom: 12px; margin-top: 28px;
    padding-bottom: 8px; border-bottom: 1px solid #334155;
  }
  .sidebar h3:first-of-type { margin-top: 0; }
  .contact-item { font-size: 0.85rem; color: #cbd5e1; margin-bottom: 8px; display: flex; align-items: center; gap: 8px; }
  .contact-item span { font-size: 1rem; }
  .skill-bar { margin-bottom: 10px; }
  .skill-name { font-size: 0.82rem; color: #cbd5e1; margin-bottom: 4px; display: flex; justify-content: space-between; }
  .bar-bg { height: 6px; background: #334155; border-radius: 3px; overflow: hidden; }
  .bar-fill { height: 100%; border-radius: 3px; }
  .bar-fill.w95 { width: 95%; background: #38bdf8; }
  .bar-fill.w90 { width: 90%; background: #38bdf8; }
  .bar-fill.w85 { width: 85%; background: #818cf8; }
  .bar-fill.w80 { width: 80%; background: #818cf8; }
  .bar-fill.w75 { width: 75%; background: #a78bfa; }
  .languages { font-size: 0.85rem; color: #cbd5e1; line-height: 1.8; }
  .main { padding: 40px 36px; }
  .main h2 {
    font-size: 0.78rem; text-transform: uppercase; letter-spacing: 2px;
    color: #38bdf8; margin-bottom: 20px; padding-bottom: 8px;
    border-bottom: 2px solid #e2e8f0; font-weight: 700;
  }
  .section { margin-bottom: 36px; }
  .entry { margin-bottom: 24px; }
  .entry-header { display: flex; justify-content: space-between; align-items: baseline; margin-bottom: 4px; flex-wrap: wrap; gap: 4px; }
  .entry-role { font-size: 1.05rem; font-weight: 700; }
  .entry-date { font-size: 0.82rem; color: #94a3b8; font-weight: 600; }
  .entry-company { font-size: 0.9rem; color: #64748b; margin-bottom: 8px; font-weight: 500; }
  .entry ul { list-style: none; }
  .entry li { font-size: 0.88rem; color: #475569; line-height: 1.6; padding-left: 16px; position: relative; margin-bottom: 4px; }
  .entry li::before { content: ''; position: absolute; left: 0; top: 9px; width: 5px; height: 5px; border-radius: 50%; background: #38bdf8; }
  .edu-entry { margin-bottom: 16px; }
  .edu-degree { font-weight: 700; font-size: 0.95rem; }
  .edu-school { font-size: 0.88rem; color: #64748b; }
  .edu-date { font-size: 0.82rem; color: #94a3b8; }
  .certs { list-style: none; }
  .certs li { font-size: 0.88rem; color: #475569; padding: 6px 0; border-bottom: 1px solid #f1f5f9; }
  @media (max-width: 700px) { .resume { grid-template-columns: 1fr; } }
</style>
</head>
<body>
  <div class="resume">
    <div class="sidebar">
      <div class="avatar">JB</div>
      <div class="name">Jordan Blake</div>
      <div class="title">Senior Software Engineer</div>
      <h3>Contact</h3>
      <div class="contact-item"><span>&#x1F4E7;</span> jordan@example.dev</div>
      <div class="contact-item"><span>&#x1F4F1;</span> +1 (555) 234-5678</div>
      <div class="contact-item"><span>&#x1F4CD;</span> Seattle, WA</div>
      <div class="contact-item"><span>&#x1F310;</span> jordanblake.dev</div>
      <h3>Skills</h3>
      <div class="skill-bar"><div class="skill-name"><span>Rust</span><span>95%</span></div><div class="bar-bg"><div class="bar-fill w95"></div></div></div>
      <div class="skill-bar"><div class="skill-name"><span>TypeScript</span><span>90%</span></div><div class="bar-bg"><div class="bar-fill w90"></div></div></div>
      <div class="skill-bar"><div class="skill-name"><span>System Design</span><span>85%</span></div><div class="bar-bg"><div class="bar-fill w85"></div></div></div>
      <div class="skill-bar"><div class="skill-name"><span>Kubernetes</span><span>80%</span></div><div class="bar-bg"><div class="bar-fill w80"></div></div></div>
      <div class="skill-bar"><div class="skill-name"><span>Machine Learning</span><span>75%</span></div><div class="bar-bg"><div class="bar-fill w75"></div></div></div>
      <h3>Languages</h3>
      <div class="languages">English (Native)<br>Spanish (Professional)<br>Japanese (Conversational)</div>
    </div>
    <div class="main">
      <div class="section">
        <h2>Experience</h2>
        <div class="entry">
          <div class="entry-header"><span class="entry-role">Senior Software Engineer</span><span class="entry-date">2023 — Present</span></div>
          <div class="entry-company">NovaTech Systems</div>
          <ul>
            <li>Architected distributed event-processing pipeline handling 2M events/sec with sub-10ms p99 latency</li>
            <li>Led migration from monolith to microservices, reducing deployment time from 45min to 3min</li>
            <li>Mentored team of 6 engineers; established code review and testing practices</li>
          </ul>
        </div>
        <div class="entry">
          <div class="entry-header"><span class="entry-role">Software Engineer II</span><span class="entry-date">2020 — 2023</span></div>
          <div class="entry-company">Cloudforge Inc.</div>
          <ul>
            <li>Built real-time collaboration engine using CRDTs, serving 50K concurrent users</li>
            <li>Optimized database queries reducing API response times by 70%</li>
            <li>Designed and shipped internal developer platform used by 200+ engineers</li>
          </ul>
        </div>
        <div class="entry">
          <div class="entry-header"><span class="entry-role">Junior Developer</span><span class="entry-date">2018 — 2020</span></div>
          <div class="entry-company">StartupHub</div>
          <ul>
            <li>Developed customer-facing dashboard with React; grew MAU from 5K to 80K</li>
            <li>Implemented CI/CD pipeline reducing release cycle from weekly to daily</li>
          </ul>
        </div>
      </div>
      <div class="section">
        <h2>Education</h2>
        <div class="edu-entry">
          <div class="edu-degree">B.S. Computer Science</div>
          <div class="edu-school">University of Washington</div>
          <div class="edu-date">2014 — 2018 | GPA: 3.8</div>
        </div>
      </div>
      <div class="section">
        <h2>Certifications</h2>
        <ul class="certs">
          <li>AWS Solutions Architect Professional (2024)</li>
          <li>Certified Kubernetes Administrator (2023)</li>
          <li>Google Cloud Professional Data Engineer (2022)</li>
        </ul>
      </div>
    </div>
  </div>
</body>
</html>"##;

#[no_mangle]
pub extern "C" fn x402_handle() {
    host_log(1, "resume_page: serving resume");
    respond(200, BODY, "text/html; charset=utf-8");
}
