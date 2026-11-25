use super::*;

#[tokio::test]
async fn a_point_in_a_cached_city_and_postal_code_needs_no_lookup() {
    let s = setup().await;
    cache_area(&s.db, "osm-relation-10", AreaKind::City, 0.1).await;
    cache_area(&s.db, "osm-relation-20", AreaKind::PostalCode, 0.02).await;

    let here = s.directory.locate(50.881, 4.701).await.unwrap();
    assert_eq!(here.city.unwrap().id, "osm-relation-10");
    assert_eq!(here.postal_code.unwrap().id, "osm-relation-20");
    assert_eq!(s.inner.asked.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn the_smallest_cached_area_of_a_kind_wins() {
    let s = setup().await;
    cache_area(&s.db, "osm-relation-10", AreaKind::City, 0.3).await;
    cache_area(&s.db, "osm-relation-12", AreaKind::City, 0.1).await;
    cache_area(&s.db, "osm-relation-11", AreaKind::City, 0.1).await;
    cache_area(&s.db, "osm-relation-20", AreaKind::PostalCode, 0.02).await;

    let here = s.directory.locate(50.881, 4.701).await.unwrap();
    assert_eq!(here.city.unwrap().id, "osm-relation-11");
    assert_eq!(s.inner.asked.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn half_an_answer_from_the_outlines_is_not_used() {
    let s = setup().await;
    cache_area(&s.db, "osm-relation-10", AreaKind::City, 0.1).await;

    let here = s.directory.locate(50.881, 4.701).await.unwrap();
    assert_eq!(here.city.unwrap().id, "osm-relation-1", "the directory's");
    assert_eq!(s.inner.asked.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_lookup_is_remembered_for_its_cell() {
    let s = setup().await;
    s.directory.locate(50.8812, 4.7012).await.unwrap();
    assert_eq!(s.inner.asked.load(Ordering::SeqCst), 1);

    let again = s.directory.locate(50.8814, 4.7009).await.unwrap();
    assert_eq!(again.city.unwrap().id, "osm-relation-1");
    assert_eq!(s.inner.asked.load(Ordering::SeqCst), 1, "same cell");

    s.directory.locate(50.8830, 4.7012).await.unwrap();
    assert_eq!(s.inner.asked.load(Ordering::SeqCst), 2, "another cell");
}

#[tokio::test]
async fn an_error_is_not_remembered() {
    let s = setup().await;
    s.inner.down.store(true, Ordering::SeqCst);
    assert!(s.directory.locate(50.881, 4.701).await.is_err());

    s.inner.down.store(false, Ordering::SeqCst);
    assert!(s.directory.locate(50.881, 4.701).await.is_ok());
    assert_eq!(s.inner.asked.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn an_answer_expires_after_a_day() {
    let s = setup().await;
    s.directory.locate(50.881, 4.701).await.unwrap();
    s.now.fetch_add(TTL_SECS - 1, Ordering::SeqCst);
    s.directory.locate(50.881, 4.701).await.unwrap();
    assert_eq!(s.inner.asked.load(Ordering::SeqCst), 1);

    s.now.fetch_add(1, Ordering::SeqCst);
    s.directory.locate(50.881, 4.701).await.unwrap();
    assert_eq!(s.inner.asked.load(Ordering::SeqCst), 2);
}
