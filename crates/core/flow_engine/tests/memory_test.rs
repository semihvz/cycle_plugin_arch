use flow_engine::memory::{DataStream, MemoryRouter};
use std::sync::Arc;
use std::thread;

#[test]
fn test_data_stream_creation_and_update() {
    let stream = DataStream::new("test_stream".to_string());
    assert_eq!(stream.name, "test_stream");
    assert_eq!(stream.last_updated.load(std::sync::atomic::Ordering::Relaxed), 0);

    {
        let mut guard = stream.data.write().unwrap();
        guard.extend_from_slice(b"hello world");
    }

    let guard = stream.data.read().unwrap();
    assert_eq!(guard.as_slice(), b"hello world");
}

#[test]
fn test_memory_router_get_or_create() {
    let router = MemoryRouter::new();

    // Non-existent stream should return None from get_stream
    assert!(router.get_stream("stream_a").is_none());

    // Creating stream
    let s1 = router.get_or_create_stream("stream_a");
    assert_eq!(s1.name, "stream_a");

    // Existing stream should return the same Arc instance
    let s2 = router.get_stream("stream_a");
    assert!(s2.is_some());
    assert!(Arc::ptr_eq(&s1, &s2.unwrap()));
}

#[test]
fn test_memory_router_concurrent_access() {
    let router = Arc::new(MemoryRouter::new());
    let mut handles = vec![];

    for i in 0..10 {
        let router_clone = router.clone();
        let handle = thread::spawn(move || {
            let stream_name = format!("stream_{}", i % 3);
            let stream = router_clone.get_or_create_stream(&stream_name);
            let mut data = stream.data.write().unwrap();
            data.extend_from_slice(format!("data_{}", i).as_bytes());
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let s0 = router.get_stream("stream_0");
    assert!(s0.is_some());
    assert!(!s0.unwrap().data.read().unwrap().is_empty());
}
