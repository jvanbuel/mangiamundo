//! Countries to give added places, by what they would bring to Leuven.

use axum::Router;
use serde_json::Value;

use crate::common::{call, HERE};

/// Countries by what they would bring to Leuven: one already there, one new from a
/// continent that is there, and two new from a continent that isn't.
/// In that order.
pub async fn countries_to_add(app: &Router) -> (String, String, String, String) {
    let (_, discover) = call(app, "GET", &format!("/api/discover?{HERE}"), None, None).await;
    let here: Vec<&str> = discover["countries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["code"].as_str().unwrap())
        .collect();
    let continents_here: Vec<&str> = discover["countries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["continent"].as_str().unwrap())
        .collect();
    let (_, catalog) = call(app, "GET", "/api/countries", None, None).await;
    let addable: Vec<&Value> = catalog["countries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| !c["cuisine"].is_null())
        .collect();
    let code = |c: &&Value| c["code"].as_str().unwrap().to_string();
    let present = addable
        .iter()
        .find(|c| here.contains(&c["code"].as_str().unwrap()))
        .map(code)
        .unwrap();
    let new_country = addable
        .iter()
        .find(|c| {
            !here.contains(&c["code"].as_str().unwrap())
                && continents_here.contains(&c["continent"].as_str().unwrap())
        })
        .map(code)
        .unwrap();
    let elsewhere: Vec<&&Value> = addable
        .iter()
        .filter(|c| !continents_here.contains(&c["continent"].as_str().unwrap()))
        .collect();
    let far = elsewhere
        .iter()
        .find(|c| {
            elsewhere
                .iter()
                .any(|o| o["continent"] == c["continent"] && o["code"] != c["code"])
        })
        .unwrap();
    let (far_a, far_b) = (
        code(far),
        elsewhere
            .iter()
            .find(|o| o["continent"] == far["continent"] && o["code"] != far["code"])
            .map(|c| code(c))
            .unwrap(),
    );
    (present, new_country, far_a, far_b)
}
