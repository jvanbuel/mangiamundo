use super::*;

/// A fake Overpass server: answers with `status` and one restaurant after `delay`.
async fn fake_server(status: u16, delay: Duration) -> String {
    use axum::http::StatusCode;
    let app = axum::Router::new().route(
        "/api/interpreter",
        axum::routing::post(move || async move {
            tokio::time::sleep(delay).await;
            let body = r#"{"elements":[{"type":"node","id":1,"lat":50.88,"lon":4.70,"tags":{"name":"Lalibela","cuisine":"ethiopian"}}]}"#;
            (StatusCode::from_u16(status).unwrap(), body)
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/api/interpreter", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}

#[tokio::test]
async fn a_failing_or_slow_server_brings_in_the_next_one() {
    let failing = fake_server(502, Duration::ZERO).await;
    let slow = fake_server(200, Duration::from_secs(5)).await;
    let quick = fake_server(200, Duration::from_millis(10)).await;
    let overpass = Overpass::new(reqwest::Client::new(), vec![failing, slow, quick]);
    let asked = std::sync::atomic::AtomicUsize::new(0);
    let started = Instant::now();
    let places = overpass
        .fetch_any(
            "q",
            Duration::from_secs(10),
            Some(Duration::from_millis(200)),
            &|| {
                asked.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            },
        )
        .await
        .unwrap();
    assert_eq!(places[0].id, "osm-node-1");
    // The 502 brings in the slow server at once; 200 ms later the quick one answers first.
    assert_eq!(asked.into_inner(), 2);
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn without_a_deadline_servers_are_asked_one_at_a_time() {
    let slow = fake_server(200, Duration::from_millis(500)).await;
    let quick = fake_server(200, Duration::ZERO).await;
    let overpass = Overpass::new(reqwest::Client::new(), vec![slow, quick]);
    let asked = std::sync::atomic::AtomicUsize::new(0);
    overpass
        .fetch_any("q", Duration::from_secs(10), None, &|| {
            asked.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        })
        .await
        .unwrap();
    assert_eq!(asked.into_inner(), 0);
}

#[test]
fn the_expected_time_follows_measured_queries() {
    let overpass = Overpass::new(reqwest::Client::new(), vec![]);
    for _ in 0..20 {
        overpass.measured(100.0);
    }
    assert!((99.0..=100.0).contains(&*overpass.usual_secs.lock().unwrap()));
}
