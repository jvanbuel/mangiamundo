use super::*;

#[tokio::test]
async fn a_search_is_remembered_however_it_is_spelled() {
    let s = setup().await;
    let found = s.directory.search("Leuven").await.unwrap();
    assert_eq!(found[0].id, "osm-relation-1");
    assert_eq!(s.directory.search("  leuven ").await.unwrap(), found);
    assert_eq!(s.directory.search("LEUVEN").await.unwrap(), found);
    assert_eq!(s.inner.asked.load(Ordering::SeqCst), 1);

    s.directory.search("Sint Niklaas").await.unwrap();
    s.directory.search("sint  niklaas").await.unwrap();
    assert_eq!(s.inner.asked.load(Ordering::SeqCst), 2);

    s.now.fetch_add(TTL_SECS, Ordering::SeqCst);
    s.directory.search("leuven").await.unwrap();
    assert_eq!(s.inner.asked.load(Ordering::SeqCst), 3, "a day later");
}

#[tokio::test]
async fn a_search_that_found_nothing_is_remembered_for_minutes() {
    let s = setup().await;
    assert_eq!(
        s.directory.search("nowhere").await.unwrap(),
        [] as [AreaInfo; 0]
    );
    s.now.fetch_add(NOTHING_TTL_SECS - 1, Ordering::SeqCst);
    assert_eq!(
        s.directory.search("nowhere").await.unwrap(),
        [] as [AreaInfo; 0]
    );
    assert_eq!(s.inner.asked.load(Ordering::SeqCst), 1);

    s.now.fetch_add(1, Ordering::SeqCst);
    s.directory.search("nowhere").await.unwrap();
    assert_eq!(s.inner.asked.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn a_failed_or_very_long_search_is_not_remembered() {
    let s = setup().await;
    s.inner.down.store(true, Ordering::SeqCst);
    assert!(s.directory.search("leuven").await.is_err());
    s.inner.down.store(false, Ordering::SeqCst);
    assert_ne!(
        s.directory.search("leuven").await.unwrap(),
        [] as [AreaInfo; 0]
    );
    assert_eq!(s.inner.asked.load(Ordering::SeqCst), 2);

    let long = "a".repeat(MAX_QUERY_BYTES + 1);
    s.directory.search(&long).await.unwrap();
    s.directory.search(&long).await.unwrap();
    assert_eq!(s.inner.asked.load(Ordering::SeqCst), 4);
}
