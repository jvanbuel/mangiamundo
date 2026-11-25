//! Suggesting a place to eat.

/// A place that can be suggested: the countries it serves, at least one, and how far it is.
pub struct Candidate<'a> {
    pub countries: &'a [String],
    pub distance_m: u32,
}

/// Picks one of `candidates` at random: its index, and the country the globe lands on,
/// an untasted one when the place serves several. Countries not in `tasted` and nearer
/// places are more likely. None when there are no candidates.
pub fn suggest(candidates: &[Candidate], tasted: &[String]) -> Option<(usize, String)> {
    if candidates.is_empty() {
        return None;
    }
    let weights: Vec<f64> = candidates
        .iter()
        .map(|c| suggestion_weight(c, tasted))
        .collect();
    let index = pick(&weights, random_unit());
    let countries = candidates[index].countries;
    let country = countries
        .iter()
        .find(|c| !tasted.contains(c))
        .or(countries.first())?;
    Some((index, country.clone()))
}

/// How likely a place is suggested: three times as likely when it serves a country not
/// tasted yet, and halving every 2 km away.
// ponytail: no levels (#72) or quests (#73) in the weighting yet; add them here.
pub(super) fn suggestion_weight(place: &Candidate, tasted: &[String]) -> f64 {
    let new = place.countries.iter().any(|c| !tasted.contains(c));
    let near = 0.5f64.powf(f64::from(place.distance_m) / 2000.0);
    if new {
        3.0 * near
    } else {
        near
    }
}

/// The index `r` (in `[0, 1)`) lands on when each index takes its weight's share of the line.
pub(super) fn pick(weights: &[f64], r: f64) -> usize {
    let mut left = r * weights.iter().sum::<f64>();
    for (i, w) in weights.iter().enumerate() {
        if left < *w {
            return i;
        }
        left -= w;
    }
    weights.len() - 1
}

pub(super) fn random_unit() -> f64 {
    let mut bytes = [0u8; 8];
    getrandom::fill(&mut bytes).expect("the system has no randomness");
    #[expect(clippy::cast_precision_loss, reason = "53 bits fit an f64 exactly")]
    let unit = (u64::from_le_bytes(bytes) >> 11) as f64 / (1u64 << 53) as f64;
    unit
}
