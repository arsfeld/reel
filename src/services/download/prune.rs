//! Budget-driven prune selection.
//!
//! Pure selection of which completed downloads to remove when total usage
//! exceeds the configured budget. Only *watched* downloads are eligible, oldest
//! first; unwatched downloads are never selected (R19). The currently-playing
//! item is never selected even if watched (its file must not be deleted out
//! from under the player). The caller performs the actual file deletes and repo
//! updates; this function only decides.

/// A completed download considered for pruning. The caller builds these by
/// joining `downloads` with watch state (`watched` / `last_watched_at`).
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct PruneCandidate {
    pub media_item_id: String,
    pub size_bytes: u64,
    pub watched: bool,
    /// When last watched (ISO 8601). `None` (watched with no timestamp) sorts
    /// as oldest.
    pub last_watched_at: Option<String>,
    /// When the download completed (ISO 8601). Secondary sort key.
    pub completed_at: Option<String>,
}

/// Result of a prune decision.
#[derive(Debug, Clone, PartialEq, Default)]
#[allow(dead_code)]
pub struct PruneOutcome {
    /// `media_item_id`s to prune, oldest watched first.
    pub to_prune: Vec<String>,
    /// True when usage is still over budget after pruning every eligible
    /// watched download — i.e. only unwatched downloads remain over budget and
    /// the user should be warned instead of having media auto-deleted.
    pub warn: bool,
}

/// Select downloads to prune so total usage drops to or below `budget`.
///
/// Eligible = completed, `watched`, and not the `now_playing` item. Eligible
/// candidates are ordered oldest-first by `last_watched_at` (absent = oldest),
/// tie-broken by `completed_at` then `media_item_id` for determinism, and
/// selected until projected usage is within budget. If usage is still over
/// budget after exhausting eligible candidates, `warn` is set and nothing
/// further is selected. With no budget, nothing is ever pruned.
#[allow(dead_code)]
pub fn select_prune_candidates(
    candidates: &[PruneCandidate],
    total_bytes: u64,
    budget: Option<u64>,
    now_playing_id: Option<&str>,
) -> PruneOutcome {
    let Some(budget) = budget else {
        return PruneOutcome::default();
    };
    if total_bytes <= budget {
        return PruneOutcome::default();
    }

    // Eligible: watched, not currently playing.
    let mut eligible: Vec<&PruneCandidate> = candidates
        .iter()
        .filter(|c| c.watched && Some(c.media_item_id.as_str()) != now_playing_id)
        .collect();

    // Oldest first. Absent last_watched_at / completed_at sort first via "".
    eligible.sort_by(|a, b| {
        let ka = (
            a.last_watched_at.as_deref().unwrap_or(""),
            a.completed_at.as_deref().unwrap_or(""),
            a.media_item_id.as_str(),
        );
        let kb = (
            b.last_watched_at.as_deref().unwrap_or(""),
            b.completed_at.as_deref().unwrap_or(""),
            b.media_item_id.as_str(),
        );
        ka.cmp(&kb)
    });

    let mut running = total_bytes;
    let mut to_prune = Vec::new();
    for c in eligible {
        if running <= budget {
            break;
        }
        running = running.saturating_sub(c.size_bytes);
        to_prune.push(c.media_item_id.clone());
    }

    PruneOutcome {
        to_prune,
        warn: running > budget,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(id: &str, size: u64, watched: bool, last: Option<&str>) -> PruneCandidate {
        PruneCandidate {
            media_item_id: id.to_string(),
            size_bytes: size,
            watched,
            last_watched_at: last.map(String::from),
            completed_at: Some("2026-05-01T00:00:00Z".to_string()),
        }
    }

    #[test]
    fn no_budget_never_prunes() {
        let c = [cand("a", 100, true, Some("2026-01-01"))];
        let out = select_prune_candidates(&c, 1_000, None, None);
        assert_eq!(out, PruneOutcome::default());
    }

    #[test]
    fn under_budget_does_nothing() {
        let c = [cand("a", 100, true, Some("2026-01-01"))];
        let out = select_prune_candidates(&c, 100, Some(200), None);
        assert!(out.to_prune.is_empty());
        assert!(!out.warn);
    }

    #[test]
    fn all_unwatched_over_budget_warns_and_prunes_nothing() {
        // Covers AE5: over budget, all unwatched -> nothing deleted, warn.
        let c = [cand("a", 600, false, None), cand("b", 600, false, None)];
        let out = select_prune_candidates(&c, 1_200, Some(500), None);
        assert!(out.to_prune.is_empty());
        assert!(out.warn);
    }

    #[test]
    fn prunes_oldest_watched_until_under_budget() {
        let c = [
            cand("new", 400, true, Some("2026-05-10")),
            cand("old", 400, true, Some("2026-01-01")),
            cand("mid", 400, true, Some("2026-03-01")),
        ];
        // total 1200, budget 500 -> must drop 700+ -> prune oldest two.
        let out = select_prune_candidates(&c, 1_200, Some(500), None);
        assert_eq!(out.to_prune, vec!["old", "mid"]);
        assert!(!out.warn);
    }

    #[test]
    fn now_playing_never_pruned_even_if_oldest_watched() {
        let c = [
            cand("playing", 800, true, Some("2026-01-01")), // oldest, but playing
            cand("other", 800, true, Some("2026-05-01")),
        ];
        let out = select_prune_candidates(&c, 1_600, Some(500), Some("playing"));
        // 'playing' excluded; only 'other' pruned (1600-800=800 still > 500 -> warn).
        assert_eq!(out.to_prune, vec!["other"]);
        assert!(out.warn);
    }

    #[test]
    fn only_watched_are_candidates_stop_when_under() {
        let c = [
            cand("w_old", 400, true, Some("2026-01-01")),
            cand("u", 400, false, None),
            cand("w_new", 400, true, Some("2026-05-01")),
        ];
        // total 1200, budget 800 -> dropping the oldest watched (->800) is
        // exactly within budget, so pruning stops after one.
        let out = select_prune_candidates(&c, 1_200, Some(800), None);
        assert_eq!(out.to_prune, vec!["w_old"]);
        assert!(!out.warn);
    }

    #[test]
    fn tie_in_last_watched_breaks_on_completed_then_id() {
        let a = PruneCandidate {
            media_item_id: "a".into(),
            size_bytes: 400,
            watched: true,
            last_watched_at: Some("2026-01-01".into()),
            completed_at: Some("2026-01-05".into()),
        };
        let b = PruneCandidate {
            media_item_id: "b".into(),
            size_bytes: 400,
            watched: true,
            last_watched_at: Some("2026-01-01".into()), // tie
            completed_at: Some("2026-01-02".into()),    // earlier completion
        };
        // Need to drop one; b completed earlier -> pruned first.
        let out = select_prune_candidates(&[a, b], 800, Some(500), None);
        assert_eq!(out.to_prune, vec!["b"]);
    }

    #[test]
    fn watched_with_empty_last_watched_is_oldest() {
        let c = [
            cand("dated", 400, true, Some("2026-05-01")),
            cand("undated", 400, true, None), // treated as oldest
        ];
        let out = select_prune_candidates(&c, 800, Some(500), None);
        assert_eq!(out.to_prune, vec!["undated"]);
    }
}
