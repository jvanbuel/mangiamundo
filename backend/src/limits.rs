//! Rate limits, and who the request being handled came from so limits counted below the
//! handlers (such as in the area directory) know whom to count against.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

tokio::task_local! {
    /// Who sent the request being handled, for limits counted below the handlers.
    static CLIENT: String;
}

/// Runs `work` as if a request from `client` were being handled.
pub async fn as_client<F: std::future::Future>(client: String, work: F) -> F::Output {
    CLIENT.scope(client, work).await
}

/// Who sent the request being handled, or `None` in background work.
pub fn client() -> Option<String> {
    CLIENT.try_with(Clone::clone).ok()
}

/// Recent requests per user and kind, for rate limits. In memory: the server runs as one
/// machine, and a restart forgetting them is harmless.
#[derive(Clone, Default)]
pub struct Limits(Arc<Mutex<HashMap<LimitKey, Vec<i64>>>>);

/// A user id and the kind of request.
type LimitKey = (String, &'static str);

impl Limits {
    /// Counts the request and returns true when `user` made fewer than `max` requests of this
    /// kind in the `window` seconds before `now`.
    pub fn allow(&self, user: &str, kind: &'static str, max: usize, window: i64, now: i64) -> bool {
        let mut all = self.0.lock().unwrap();
        // ponytail: entries of users who stopped asking stay until restart; a few bytes each.
        let times = all.entry((user.to_string(), kind)).or_default();
        times.retain(|&at| at > now - window);
        let allowed = times.len() < max;
        if allowed {
            times.push(now);
        }
        allowed
    }

    /// Like `allow`, for several budgets at once, each a `(max, window)`: the request is
    /// counted, and true returned, when `user` is within all of them.
    pub fn allow_each(
        &self,
        user: &str,
        kind: &'static str,
        budgets: &[(usize, i64)],
        now: i64,
    ) -> bool {
        let mut all = self.0.lock().unwrap();
        let times = all.entry((user.to_string(), kind)).or_default();
        let longest = budgets.iter().map(|&(_, window)| window).max().unwrap_or(0);
        times.retain(|&at| at > now - longest);
        let allowed = budgets
            .iter()
            .all(|&(max, window)| times.iter().filter(|&&at| at > now - window).count() < max);
        if allowed {
            times.push(now);
        }
        allowed
    }
}
