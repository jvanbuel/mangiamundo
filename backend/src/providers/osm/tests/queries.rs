use super::*;

#[test]
fn element_queries_take_only_osm_element_ids() {
    let q = element_query("osm-way-1085544575").unwrap();
    assert!(q.contains("way(id:1085544575)[\"amenity\"~"), "{q}");
    assert!(element_query("osm-node-12")
        .unwrap()
        .contains("node(id:12)"));
    for bad in [
        "user-abc",
        "demo-leuven-1",
        "osm-area-3",
        "osm-node-1;out",
        "osm-node-",
    ] {
        assert_eq!(element_query(bad), None, "{bad}");
    }
}

#[test]
fn queries_the_bounding_box() {
    let b = Bbox {
        south: 50.82,
        west: 4.64,
        north: 50.94,
        east: 4.77,
    };
    assert!(query(b, 60).contains("(50.82,4.64,50.94,4.77)"));
    let precise = Bbox {
        south: 50.824_208_1,
        west: 4.000_006,
        north: 50.944_073_7,
        east: 4.770_424_8,
    };
    assert!(
        query(precise, 60).contains("(50.8242081,4.000006,50.9440737,4.7704248)"),
        "no rounding"
    );
}
