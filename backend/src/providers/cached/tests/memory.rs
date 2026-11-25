use super::*;

#[tokio::test]
async fn a_full_memory_drops_the_answer_that_expires_first() {
    let memory = Memory::new(3);
    for i in 0..=3 {
        memory.put(i, i, i64::from(i), TTL_SECS);
    }
    assert_eq!(memory.kept.lock().unwrap().len(), 3);
    assert_eq!(memory.get(&0, 3), None);
    assert_eq!(memory.get(&1, 3), Some(1));

    // What has expired goes before anything fresh does.
    memory.put(4, 4, TTL_SECS + 2, TTL_SECS);
    assert_eq!(memory.kept.lock().unwrap().len(), 2);
    assert_eq!(memory.get(&3, TTL_SECS + 2), Some(3));
}
