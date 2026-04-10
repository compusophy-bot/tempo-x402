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

const FEED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom" xmlns:dc="http://purl.org/dc/elements/1.1/">
  <channel>
    <title>Nexus Engineering Blog</title>
    <link>https://blog.nexus.example.com</link>
    <description>Technical insights, product updates, and engineering deep-dives from the Nexus team.</description>
    <language>en-us</language>
    <lastBuildDate>Thu, 10 Apr 2026 12:00:00 +0000</lastBuildDate>
    <atom:link href="https://blog.nexus.example.com/feed.xml" rel="self" type="application/rss+xml"/>
    <image>
      <url>https://blog.nexus.example.com/images/logo.png</url>
      <title>Nexus Engineering Blog</title>
      <link>https://blog.nexus.example.com</link>
    </image>

    <item>
      <title>Introducing Our V2 API: Faster, Simpler, Better</title>
      <link>https://blog.nexus.example.com/2026/04/introducing-v2-api</link>
      <guid isPermaLink="true">https://blog.nexus.example.com/2026/04/introducing-v2-api</guid>
      <pubDate>Wed, 09 Apr 2026 09:00:00 +0000</pubDate>
      <dc:creator>Alice Kim</dc:creator>
      <category>Product</category>
      <category>API</category>
      <description>&lt;p&gt;Today we are launching the V2 API with a completely redesigned request format, 3x faster response times, and built-in pagination. The new API is backward compatible through our migration layer, so existing integrations continue to work while you upgrade at your own pace.&lt;/p&gt;&lt;p&gt;Key improvements include streaming responses, webhook delivery guarantees, and a unified error format across all endpoints.&lt;/p&gt;</description>
    </item>

    <item>
      <title>How We Scaled to 1 Million Requests Per Second</title>
      <link>https://blog.nexus.example.com/2026/04/scaling-million-rps</link>
      <guid isPermaLink="true">https://blog.nexus.example.com/2026/04/scaling-million-rps</guid>
      <pubDate>Mon, 07 Apr 2026 14:30:00 +0000</pubDate>
      <dc:creator>Marcus Rivera</dc:creator>
      <category>Engineering</category>
      <category>Infrastructure</category>
      <description>&lt;p&gt;Our journey from 10K to 1M RPS involved rewriting our hot path in Rust, adopting io_uring for async I/O, and implementing a custom connection pooling layer. This post walks through the architecture decisions, the benchmarks that guided us, and the surprising bottlenecks we discovered along the way.&lt;/p&gt;</description>
    </item>

    <item>
      <title>Security Best Practices for API Key Management</title>
      <link>https://blog.nexus.example.com/2026/03/api-key-security</link>
      <guid isPermaLink="true">https://blog.nexus.example.com/2026/03/api-key-security</guid>
      <pubDate>Thu, 28 Mar 2026 10:00:00 +0000</pubDate>
      <dc:creator>David Wright</dc:creator>
      <category>Security</category>
      <description>&lt;p&gt;API keys are the front door to your infrastructure. In this guide, we cover rotation strategies, scoped permissions, environment variable management, and monitoring for leaked credentials. We also introduce our new key audit dashboard that tracks usage patterns and flags anomalies.&lt;/p&gt;</description>
    </item>

    <item>
      <title>Building Real-Time Dashboards with Server-Sent Events</title>
      <link>https://blog.nexus.example.com/2026/03/sse-dashboards</link>
      <guid isPermaLink="true">https://blog.nexus.example.com/2026/03/sse-dashboards</guid>
      <pubDate>Tue, 18 Mar 2026 11:00:00 +0000</pubDate>
      <dc:creator>Sarah Patel</dc:creator>
      <category>Frontend</category>
      <category>Tutorial</category>
      <description>&lt;p&gt;WebSockets are not always the right choice. Server-Sent Events offer a simpler, HTTP-native alternative for unidirectional data streams. This tutorial shows how we built our real-time analytics dashboard using SSE, with automatic reconnection, event buffering, and graceful degradation for older browsers.&lt;/p&gt;</description>
    </item>

    <item>
      <title>Why We Chose Rust for Our Core Infrastructure</title>
      <link>https://blog.nexus.example.com/2026/03/why-rust</link>
      <guid isPermaLink="true">https://blog.nexus.example.com/2026/03/why-rust</guid>
      <pubDate>Mon, 10 Mar 2026 08:00:00 +0000</pubDate>
      <dc:creator>James Chen</dc:creator>
      <category>Engineering</category>
      <category>Rust</category>
      <description>&lt;p&gt;After running Go in production for three years, we made the decision to rewrite our core services in Rust. This post covers our evaluation process, the migration strategy, what went well, what was painful, and the measurable improvements in latency, memory usage, and reliability we observed post-migration.&lt;/p&gt;</description>
    </item>

    <item>
      <title>Designing APIs That Developers Love</title>
      <link>https://blog.nexus.example.com/2026/02/api-design-principles</link>
      <guid isPermaLink="true">https://blog.nexus.example.com/2026/02/api-design-principles</guid>
      <pubDate>Wed, 26 Feb 2026 13:00:00 +0000</pubDate>
      <dc:creator>Lena Olsson</dc:creator>
      <category>Product</category>
      <category>Developer Experience</category>
      <description>&lt;p&gt;Great API design is invisible. In this post, we share the principles that guide our API decisions: consistent naming conventions, predictable pagination, meaningful error messages, sensible defaults, and comprehensive examples. We also discuss how we gather developer feedback and iterate on our API surface.&lt;/p&gt;</description>
    </item>
  </channel>
</rss>"#;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    host_log(1, "serving rss feed");
    respond(200, FEED, "application/xml");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
