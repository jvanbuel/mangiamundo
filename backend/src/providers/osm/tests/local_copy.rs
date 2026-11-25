use super::*;

#[test]
fn coverage_needs_the_whole_area_inside_the_box() {
    let b = |south, west, north, east| Bbox {
        south,
        west,
        north,
        east,
    };
    assert!(covered(b(50.84, 4.65, 50.92, 4.76))); // Leuven
    assert!(covered(b(50.58, 2.97, 50.68, 3.13))); // Lille
    assert!(!covered(b(49.3, 5.0, 49.6, 5.3))); // crosses the southern edge
    assert!(!covered(b(48.81, 2.22, 48.90, 2.47))); // Paris
}
