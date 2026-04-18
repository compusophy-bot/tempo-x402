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

const REPORT: &str = r#"
================================================================================
                    NEXUS PLATFORM - MONTHLY OPERATIONS REPORT
                           Period: March 1-31, 2026
                        Generated: April 10, 2026 14:00 UTC
================================================================================

1. EXECUTIVE SUMMARY
--------------------------------------------------------------------------------
Overall platform health: GOOD. Uptime target of 99.95% exceeded at 99.97%.
Revenue grew 12.5% month-over-month. Three minor incidents, all resolved within
SLA. One new enterprise customer onboarded (Weyland-Yutani Corp).

2. INFRASTRUCTURE METRICS
--------------------------------------------------------------------------------
  Metric                     March 2026    Feb 2026    Change
  -------------------------  ----------    --------    ------
  Total Requests             84.2M         78.1M       +7.8%
  Avg Latency (p50)          12ms          14ms        -14.3%
  Avg Latency (p95)          45ms          52ms        -13.5%
  Avg Latency (p99)          128ms         145ms       -11.7%
  Error Rate                 0.023%        0.031%      -25.8%
  Uptime                     99.97%        99.94%      +0.03%
  Peak Concurrent Conn.      24,812        21,430      +15.8%
  Bandwidth (egress)         2.4 TB        2.1 TB      +14.3%

3. COMPUTE RESOURCES
--------------------------------------------------------------------------------
  Resource                   Allocated    Used (avg)   Used (peak)  Utilization
  -------------------------  ---------    ----------   -----------  -----------
  CPU Cores                  128          46           98           36% / 77%
  Memory (GB)                512          184          342          36% / 67%
  Storage (TB)               20           8.4          --           42%
  GPU (A100 units)           8            3.2          7.1          40% / 89%

4. REVENUE BREAKDOWN
--------------------------------------------------------------------------------
  Category                   Amount       % of Total   MoM Change
  -------------------------  ----------   ----------   ----------
  Enterprise Subscriptions   $42,180      48.2%        +8.3%
  Pro Subscriptions          $28,420      32.5%        +15.1%
  API Usage (overage)        $12,840      14.7%        +18.6%
  Professional Services      $4,050       4.6%         +2.1%
  -------------------------  ----------   ----------
  TOTAL REVENUE              $87,490      100.0%       +12.5%

5. CUSTOMER METRICS
--------------------------------------------------------------------------------
  Metric                     Current      Previous     Change
  -------------------------  ----------   ----------   ------
  Total Customers            847          812          +35
  Enterprise                 28           27           +1
  Pro                        284          268          +16
  Free                       535          517          +18
  Monthly Churn Rate         1.2%         1.5%         -0.3%
  NPS Score                  72           68           +4
  Avg Support Response       1.8h         2.3h         -21.7%
  Open Support Tickets       14           22           -36.4%

6. INCIDENTS
--------------------------------------------------------------------------------
  ID            Date        Duration  Severity  Description
  -----------   ----------  --------  --------  --------------------------------
  INC-0308-001  Mar 08      23min     Minor     Elevated API latency (EU-West)
                                                 Root cause: DNS resolver timeout
                                                 Resolution: Failover to backup
  INC-0315-002  Mar 15      8min      Minor     Webhook delivery delays
                                                 Root cause: Queue backlog spike
                                                 Resolution: Auto-scaled workers
  INC-0322-003  Mar 22      41min     Moderate  Read replica lag (US-East)
                                                 Root cause: Long-running query
                                                 Resolution: Query optimization +
                                                 statement timeout enforcement

7. SECURITY
--------------------------------------------------------------------------------
  - Vulnerability scans: 4 completed, 0 critical findings
  - Penetration test: Scheduled for April 15
  - Certificate renewals: 3 completed (wildcard, API, CDN)
  - Failed auth attempts: 12,480 (blocked by rate limiter)
  - Suspicious IPs blocked: 342
  - SOC 2 audit: In progress, expected completion May 2026

8. UPCOMING MAINTENANCE
--------------------------------------------------------------------------------
  Date          Window          Description
  ----------    -----------     ----------------------------------------
  Apr 12        02:00-04:00     Database version upgrade (zero-downtime)
  Apr 15        --              Penetration test (no user impact)
  Apr 20        03:00-03:30     CDN edge config rollout
  Apr 28        01:00-05:00     Kubernetes cluster upgrade (rolling)

================================================================================
                           END OF REPORT
  Prepared by: Platform Operations Team | ops@nexus.example.com
================================================================================
"#;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    host_log(1, "serving plain text report");
    respond(200, REPORT, "text/plain; charset=utf-8");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
