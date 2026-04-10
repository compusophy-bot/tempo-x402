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
<title>Code Snippet</title>
<style>
  *, *::before, *::after { margin: 0; padding: 0; box-sizing: border-box; }
  body { font-family: 'Segoe UI', system-ui, sans-serif; background: #0f172a; color: #e2e8f0; padding: 60px 20px; display: flex; justify-content: center; }
  .container { max-width: 720px; width: 100%; }
  h1 { font-size: 1.5rem; font-weight: 700; margin-bottom: 8px; }
  .desc { color: #94a3b8; font-size: 0.92rem; margin-bottom: 32px; }
  .code-window {
    background: #1e293b; border-radius: 12px; overflow: hidden;
    box-shadow: 0 8px 32px rgba(0,0,0,0.3); border: 1px solid #334155;
  }
  .window-bar {
    display: flex; align-items: center; padding: 12px 16px;
    background: #0f172a; border-bottom: 1px solid #334155; gap: 8px;
  }
  .dot { width: 12px; height: 12px; border-radius: 50%; }
  .dot.red { background: #ef4444; }
  .dot.yellow { background: #eab308; }
  .dot.green { background: #22c55e; }
  .file-name { margin-left: 12px; font-size: 0.82rem; color: #64748b; font-family: monospace; }
  .lang-badge { margin-left: auto; font-size: 0.72rem; padding: 2px 10px; border-radius: 4px; background: rgba(249,115,22,0.15); color: #fb923c; font-weight: 600; }
  .copy-btn {
    background: none; border: 1px solid #334155; color: #64748b; padding: 4px 12px;
    border-radius: 6px; font-size: 0.75rem; cursor: pointer; transition: all 0.2s;
  }
  .copy-btn:hover { border-color: #64748b; color: #e2e8f0; }
  .code-body { display: flex; overflow-x: auto; }
  .line-numbers {
    padding: 20px 0; text-align: right; user-select: none;
    border-right: 1px solid #334155; min-width: 52px; flex-shrink: 0;
  }
  .line-numbers span {
    display: block; padding: 0 12px; font-family: 'Cascadia Code', 'Fira Code', monospace;
    font-size: 0.82rem; line-height: 1.8; color: #475569;
  }
  .code-content { padding: 20px 20px; flex: 1; overflow-x: auto; }
  .code-content pre {
    font-family: 'Cascadia Code', 'Fira Code', monospace;
    font-size: 0.82rem; line-height: 1.8; white-space: pre;
  }
  .kw { color: #c084fc; } /* keyword */
  .ty { color: #38bdf8; } /* type */
  .fn { color: #67e8f9; } /* function */
  .str { color: #34d399; } /* string */
  .num { color: #fb923c; } /* number */
  .cm { color: #475569; font-style: italic; } /* comment */
  .op { color: #f472b6; } /* operator */
  .mac { color: #fbbf24; } /* macro */
  .lt { color: #94a3b8; } /* lifetime */
  .hl { background: rgba(56,189,248,0.08); display: inline-block; width: calc(100% + 40px); margin: 0 -20px; padding: 0 20px; }
</style>
</head>
<body>
  <div class="container">
    <h1>Syntax Highlighted Code</h1>
    <p class="desc">A Rust implementation of a concurrent task executor with work-stealing.</p>
    <div class="code-window">
      <div class="window-bar">
        <span class="dot red"></span>
        <span class="dot yellow"></span>
        <span class="dot green"></span>
        <span class="file-name">executor.rs</span>
        <span class="lang-badge">Rust</span>
        <button class="copy-btn">Copy</button>
      </div>
      <div class="code-body">
        <div class="line-numbers">
          <span>1</span><span>2</span><span>3</span><span>4</span><span>5</span>
          <span>6</span><span>7</span><span>8</span><span>9</span><span>10</span>
          <span>11</span><span>12</span><span>13</span><span>14</span><span>15</span>
          <span>16</span><span>17</span><span>18</span><span>19</span><span>20</span>
          <span>21</span><span>22</span><span>23</span><span>24</span><span>25</span>
          <span>26</span><span>27</span><span>28</span><span>29</span><span>30</span>
        </div>
        <div class="code-content"><pre><span class="kw">use</span> std::sync::{<span class="ty">Arc</span>, <span class="ty">Mutex</span>};
<span class="kw">use</span> std::collections::<span class="ty">VecDeque</span>;

<span class="cm">/// A work-stealing task executor.</span>
<span class="cm">/// Each worker maintains a local deque and can</span>
<span class="cm">/// steal tasks from other workers when idle.</span>
<span class="kw">pub struct</span> <span class="ty">Executor</span>&lt;<span class="lt">'a</span>&gt; {
    workers: <span class="ty">Vec</span>&lt;<span class="ty">Worker</span>&lt;<span class="lt">'a</span>&gt;&gt;,
    global_queue: <span class="ty">Arc</span>&lt;<span class="ty">Mutex</span>&lt;<span class="ty">VecDeque</span>&lt;<span class="ty">Task</span>&lt;<span class="lt">'a</span>&gt;&gt;&gt;&gt;,
}

<span class="kw">impl</span>&lt;<span class="lt">'a</span>&gt; <span class="ty">Executor</span>&lt;<span class="lt">'a</span>&gt; {
<span class="hl">    <span class="kw">pub fn</span> <span class="fn">new</span>(num_workers: <span class="ty">usize</span>) <span class="op">-&gt;</span> <span class="ty">Self</span> {</span>
        <span class="kw">let</span> global_queue <span class="op">=</span> <span class="ty">Arc</span>::<span class="fn">new</span>(<span class="ty">Mutex</span>::<span class="fn">new</span>(
            <span class="ty">VecDeque</span>::<span class="fn">with_capacity</span>(<span class="num">256</span>)
        ));

        <span class="kw">let</span> workers <span class="op">=</span> (<span class="num">0</span>..num_workers)
            .<span class="fn">map</span>(|id| <span class="ty">Worker</span>::<span class="fn">new</span>(id, global_queue.<span class="fn">clone</span>()))
            .<span class="fn">collect</span>();

        <span class="ty">Self</span> { workers, global_queue }
    }

    <span class="kw">pub fn</span> <span class="fn">spawn</span>(&amp;<span class="kw">self</span>, task: <span class="ty">Task</span>&lt;<span class="lt">'a</span>&gt;) {
        <span class="kw">let mut</span> queue <span class="op">=</span> <span class="kw">self</span>.global_queue.<span class="fn">lock</span>().<span class="fn">unwrap</span>();
        queue.<span class="fn">push_back</span>(task);
        <span class="mac">log!</span>(<span class="str">"Task queued, depth: {}"</span>, queue.<span class="fn">len</span>());
    }
}</pre></div>
      </div>
    </div>
  </div>
</body>
</html>"##;

#[no_mangle]
pub extern "C" fn x402_handle() {
    host_log(1, "code_snippet: serving code snippet");
    respond(200, BODY, "text/html; charset=utf-8");
}
