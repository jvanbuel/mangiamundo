mod additions;
mod imported;
mod matching;

use chrono::NaiveDate;

use crate::game::Bonus;
use crate::geo::{Bbox, Shape};
use crate::providers::FoundPlace;

use super::*;

/// A new anonymous user's id.
async fn new_user(db: &SqlitePool) -> String {
    let token = crate::account::create_user(db, None, now()).await.unwrap();
    let session = crate::account::session_for_token(db, &token, now()).await;
    session.unwrap().unwrap().user
}

fn place(id: &str, lat: f64, lng: f64, cuisine: &str) -> FoundPlace {
    FoundPlace {
        id: id.into(),
        name: id.into(),
        lat,
        lng,
        address: None,
        url: None,
        cuisine: Some(cuisine.into()),
        opening_hours: None,
        countries: vec![],
    }
}
