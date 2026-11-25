//! What a progress wheel shows while an area is being fetched.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// The step an area's fetch is in: it runs from `from` to `to` percent and should take
/// about `expected_secs`.
struct LoadStep {
    from: f64,
    to: f64,
    text: &'static str,
    started: Instant,
    expected_secs: f64,
}

/// An area being fetched: how many requests are fetching it, and the latest step.
#[derive(Default)]
struct Load {
    running: usize,
    step: Option<LoadStep>,
}

/// Areas being fetched right now, by area id. In memory: the server runs as one machine.
#[derive(Clone, Default)]
pub struct Loads(Arc<Mutex<HashMap<String, Load>>>);

impl Loads {
    /// Counts a fetch of the area until the returned guard drops, also on an error.
    pub fn begin<'a>(&'a self, id: &'a str) -> LoadDone<'a> {
        self.0
            .lock()
            .unwrap()
            .entry(id.to_string())
            .or_default()
            .running += 1;
        LoadDone(self, id)
    }

    pub fn step(&self, id: &str, from: f64, to: f64, text: &'static str, expected_secs: f64) {
        if let Some(load) = self.0.lock().unwrap().get_mut(id) {
            load.step = Some(LoadStep {
                from,
                to,
                text,
                started: Instant::now(),
                expected_secs,
            });
        }
    }

    /// Starts a new stretch of the current step from where it is now, with a new text.
    pub(super) fn restart(&self, id: &str, text: &'static str, expected_secs: f64) {
        if let Some((percent, _)) = self.progress(id) {
            self.step(id, f64::from(percent), 95.0, text, expected_secs);
        }
    }

    /// The percentage and what is happening, or None when the area isn't loading.
    pub fn progress(&self, id: &str) -> Option<(u8, &'static str)> {
        let loads = self.0.lock().unwrap();
        let step = loads.get(id)?.step.as_ref()?;
        let percent = estimate(
            step.from,
            step.to,
            step.started.elapsed().as_secs_f64(),
            step.expected_secs,
        );
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "steps run from 0 to 100"
        )]
        Some((percent as u8, step.text))
    }
}

/// Ends one fetch of an area; the progress goes when the last concurrent fetch ends.
pub struct LoadDone<'a>(&'a Loads, &'a str);

impl Drop for LoadDone<'_> {
    fn drop(&mut self) {
        let mut loads = self.0 .0.lock().unwrap();
        if let Some(load) = loads.get_mut(self.1) {
            load.running -= 1;
            if load.running == 0 {
                loads.remove(self.1);
            }
        }
    }
}

/// Progress within a step that reports none: 63% of the way after the expected time, 95%
/// after three times that, and never the end, which only the next step reaches.
fn estimate(from: f64, to: f64, elapsed_secs: f64, expected_secs: f64) -> f64 {
    from + (to - from) * (1.0 - (-elapsed_secs / expected_secs.max(0.1)).exp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_estimate_slows_down_and_never_reaches_the_end_of_its_step() {
        assert_eq!(estimate(15.0, 95.0, 0.0, 20.0), 15.0);
        let after_expected = estimate(15.0, 95.0, 20.0, 20.0);
        assert!((65.0..66.0).contains(&after_expected), "{after_expected}");
        assert!(estimate(15.0, 95.0, 600.0, 20.0) < 95.0);
    }

    #[test]
    fn progress_stays_until_the_last_concurrent_fetch_ends() {
        let loads = Loads::default();
        let first = loads.begin("leuven");
        let second = loads.begin("leuven");
        loads.step("leuven", 15.0, 95.0, "Looking for restaurants", 20.0);
        assert_eq!(
            loads.progress("leuven"),
            Some((15, "Looking for restaurants"))
        );
        drop(first);
        assert_eq!(
            loads.progress("leuven"),
            Some((15, "Looking for restaurants"))
        );
        drop(second);
        assert_eq!(loads.progress("leuven"), None);
    }
}
