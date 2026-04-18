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

const CSV_DATA: &str = "transaction_id,date,customer_name,email,product,category,quantity,unit_price,total,currency,payment_method,status,region\r\nTX-10001,2026-04-01,Acme Corporation,billing@acme.com,Enterprise Plan,Subscription,1,149.00,149.00,USD,Credit Card,Completed,US-East\r\nTX-10002,2026-04-01,Globex Industries,finance@globex.com,Pro Plan,Subscription,3,29.00,87.00,USD,PayPal,Completed,US-West\r\nTX-10003,2026-04-02,Initech LLC,ap@initech.com,API Credits,Usage,5000,0.002,10.00,USD,Credit Card,Completed,US-East\r\nTX-10004,2026-04-02,Wayne Enterprises,procurement@wayne.com,Enterprise Plan,Subscription,1,149.00,149.00,USD,Wire Transfer,Completed,US-East\r\nTX-10005,2026-04-03,Stark Industries,billing@stark.com,Enterprise Plan,Subscription,2,149.00,298.00,USD,Credit Card,Completed,US-West\r\nTX-10006,2026-04-03,Umbrella Corp,finance@umbrella.com,Pro Plan,Subscription,1,29.00,29.00,USD,Credit Card,Refunded,EU-West\r\nTX-10007,2026-04-04,Cyberdyne Systems,billing@cyberdyne.com,API Credits,Usage,10000,0.002,20.00,USD,Credit Card,Completed,US-East\r\nTX-10008,2026-04-04,Soylent Corp,ap@soylent.com,Pro Plan,Subscription,5,29.00,145.00,USD,PayPal,Completed,EU-West\r\nTX-10009,2026-04-05,Weyland-Yutani,accounts@weyland.com,Enterprise Plan,Subscription,1,149.00,149.00,USD,Wire Transfer,Pending,APAC\r\nTX-10010,2026-04-05,Oscorp Industries,billing@oscorp.com,Free Plan,Subscription,1,0.00,0.00,USD,N/A,Completed,US-East\r\nTX-10011,2026-04-06,LexCorp,finance@lexcorp.com,Pro Plan,Subscription,2,29.00,58.00,USD,Credit Card,Completed,US-East\r\nTX-10012,2026-04-06,Massive Dynamic,ap@massive.com,API Credits,Usage,25000,0.002,50.00,USD,Credit Card,Completed,EU-West\r\nTX-10013,2026-04-07,Hooli Inc,billing@hooli.com,Enterprise Plan,Subscription,1,149.00,149.00,USD,Credit Card,Completed,US-West\r\nTX-10014,2026-04-07,Pied Piper,finance@piedpiper.com,Pro Plan,Subscription,1,29.00,29.00,USD,PayPal,Completed,US-West\r\nTX-10015,2026-04-08,Dunder Mifflin,accounts@dundermifflin.com,Free Plan,Subscription,1,0.00,0.00,USD,N/A,Completed,US-East\r\nTX-10016,2026-04-08,Sterling Cooper,billing@sterlingcooper.com,Pro Plan,Subscription,4,29.00,116.00,USD,Credit Card,Completed,US-East\r\nTX-10017,2026-04-09,Prestige Worldwide,ap@prestige.com,API Credits,Usage,50000,0.002,100.00,USD,Credit Card,Completed,US-West\r\nTX-10018,2026-04-09,Wonka Industries,finance@wonka.com,Enterprise Plan,Subscription,1,149.00,149.00,USD,Wire Transfer,Completed,EU-West\r\nTX-10019,2026-04-10,Tyrell Corp,billing@tyrell.com,Pro Plan,Subscription,1,29.00,29.00,USD,Credit Card,Pending,APAC\r\nTX-10020,2026-04-10,Rekall Inc,accounts@rekall.com,API Credits,Usage,15000,0.002,30.00,USD,PayPal,Completed,APAC\r\n";

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    host_log(1, "serving csv export");
    respond(200, CSV_DATA, "text/csv");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
