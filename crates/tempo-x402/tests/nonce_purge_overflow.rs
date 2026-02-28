use tempo_x402::nonce_store::{InMemoryNonceStore, NonceStore};
use alloy::primitives::FixedBytes;
use std::sync::Arc;
use std::thread;

#[test]
fn test_purge_expired_overflow() {
    let store = Arc::new(InMemoryNonceStore::new());
    let store_clone = store.clone();
    
    // Thread 1: continually purge
    let t1 = thread::spawn(move || {
        for _ in 0..100 {
            store_clone.purge_expired(0); // Everything expires
        }
    });

    // Thread 2: continually insert
    let t2 = thread::spawn(move || {
        for i in 0..1000u64 {
            let mut bytes = [0u8; 32];
            bytes[0..8].copy_from_slice(&i.to_be_bytes());
            store.record(FixedBytes::new(bytes));
        }
    });

    t1.join().unwrap();
    t2.join().unwrap();
}
