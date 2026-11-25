use super::codes::{grouped, hash, normalized};
use super::merge::merge;
use super::sessions::device_name;
use super::*;

mod cleanup;
mod merge;
mod tokens;

#[test]
fn codes_are_grouped_and_forgiving_to_type() {
    let code = random_code(RECOVERY_LEN);
    assert_eq!(code.len(), RECOVERY_LEN);
    assert!(code.bytes().all(|b| ALPHABET.contains(&b)));
    let shown = grouped(&code);
    assert_eq!(shown.len(), RECOVERY_LEN + 4);
    assert_eq!(normalized(&shown), code);
    assert_eq!(normalized(&shown.to_lowercase().replace('-', " ")), code);
    assert_eq!(normalized("abcd-oil0"), "ABCD0110");
}

#[test]
fn device_names_are_trimmed_and_capped() {
    assert_eq!(device_name(Some("  iPhone ")), Some("iPhone".into()));
    assert_eq!(device_name(Some("   ")), None);
    assert_eq!(
        device_name(Some(&"x".repeat(100))).unwrap().len(),
        MAX_DEVICE_NAME
    );
}
