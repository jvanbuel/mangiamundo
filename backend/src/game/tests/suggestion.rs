use super::super::suggestion::{pick, random_unit, suggestion_weight};
use super::*;

#[test]
fn pick_follows_the_weights() {
    let w = [1.0, 0.0, 3.0];
    assert_eq!(pick(&w, 0.0), 0);
    assert_eq!(pick(&w, 0.24), 0);
    assert_eq!(pick(&w, 0.25), 2);
    assert_eq!(pick(&w, 0.999), 2);
    let tasted = ["JP".to_string()];
    let (pe, jp) = (["PE".to_string()], ["JP".to_string()]);
    let weight = |countries, distance_m| {
        let place = Candidate {
            countries,
            distance_m,
        };
        suggestion_weight(&place, &tasted)
    };
    assert_eq!(weight(&pe, 0), 3.0);
    assert_eq!(weight(&jp, 0), 1.0);
    assert_eq!(weight(&jp, 2000), 0.5);
    let r = random_unit();
    assert!((0.0..1.0).contains(&r));
}

#[test]
fn suggests_a_place_and_lands_on_an_untasted_country() {
    assert_eq!(suggest(&[], &[]), None);
    let tasted = ["JP".to_string()];
    let both = ["JP".to_string(), "PE".to_string()];
    let one = [Candidate {
        countries: &both,
        distance_m: 100,
    }];
    assert_eq!(suggest(&one, &tasted), Some((0, "PE".to_string())));
    let all = ["JP".to_string(), "PE".to_string()];
    assert_eq!(suggest(&one, &all), Some((0, "JP".to_string())));
}
