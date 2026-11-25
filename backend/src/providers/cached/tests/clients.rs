use super::*;

#[tokio::test]
async fn a_client_that_keeps_searching_is_refused_and_another_is_not() {
    let s = setup().await;
    let looping = "203.0.113.7".to_string();
    limits::as_client(looping.clone(), async {
        for i in 0..10 {
            s.directory.search(&format!("town {i}")).await.unwrap();
        }
        assert!(refused(s.directory.search("town 10").await));
        assert_eq!(s.inner.asked.load(Ordering::SeqCst), 10, "not asked");
        // What is remembered costs nothing, also when nothing is left.
        assert_ne!(
            s.directory.search("Town 3").await.unwrap(),
            [] as [AreaInfo; 0]
        );
    })
    .await;

    limits::as_client("198.51.100.1".into(), async {
        s.directory.search("town 10").await.unwrap();
    })
    .await;
    assert_eq!(s.inner.asked.load(Ordering::SeqCst), 11);

    // A minute later there is room again, until the hour is used up.
    limits::as_client(looping, async {
        for minute in 1..=5 {
            s.now.fetch_add(60, Ordering::SeqCst);
            for i in 0..10 {
                let query = format!("village {minute} {i}");
                s.directory.search(&query).await.unwrap();
            }
        }
        s.now.fetch_add(60, Ordering::SeqCst);
        assert!(refused(s.directory.search("hamlet").await));
        s.now.fetch_add(3600 - 6 * 60, Ordering::SeqCst);
        s.directory.search("hamlet").await.unwrap();
    })
    .await;
}

#[tokio::test]
async fn locating_and_outlines_are_counted_per_client_too() {
    let s = setup().await;
    limits::as_client("203.0.113.7".into(), async {
        for i in 0..5 {
            s.directory.locate(10.0 + f64::from(i), 4.7).await.unwrap();
        }
        assert!(refused(s.directory.locate(20.0, 4.7).await));
        // A cell someone stood in costs nothing.
        s.directory.locate(12.0, 4.7).await.unwrap();
        assert_eq!(s.inner.asked.load(Ordering::SeqCst), 5);

        // Its own count: a client that located a lot can still open an area.
        for _ in 0..20 {
            s.directory.boundary("osm-relation-5").await.unwrap();
        }
        assert!(refused(s.directory.boundary("osm-relation-5").await));
    })
    .await;
}

#[tokio::test]
async fn background_work_is_not_counted() {
    let s = setup().await;
    for i in 0..30 {
        s.directory.search(&format!("town {i}")).await.unwrap();
    }
    assert_eq!(s.inner.asked.load(Ordering::SeqCst), 30);
}
