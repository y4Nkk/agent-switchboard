//! Publisher-authored update levels carried in the release manifest and
//! resolved against the installed version. `asb_advisories` is absent from
//! manifests published before this contract; those say nothing and render no
//! level at all.

use semver::Version;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize)]
#[serde(tag = "severity", rename_all = "snake_case")]
pub(crate) enum UpdateAdvisory {
    Routine,
    #[serde(rename_all = "camelCase")]
    Important { reason_zh: String, reason_en: String },
    #[serde(rename_all = "camelCase")]
    Urgent { reason_zh: String, reason_en: String },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseAdvisory {
    severity: String,
    affected_from: String,
    fixed_in: String,
    reason_zh: String,
    reason_en: String,
}

enum Severity {
    Important,
    Urgent,
}

impl Severity {
    fn rank(&self) -> u8 {
        match self {
            Self::Important => 0,
            Self::Urgent => 1,
        }
    }
}

fn parse_stable_version(value: &str) -> Option<Version> {
    let version = Version::parse(value).ok()?;
    (version.pre.is_empty() && version.build.is_empty()).then_some(version)
}

/// A record speaks only when the candidate already contains its fix and the
/// installed version sits inside the affected range, so a user who skipped the
/// fixing release still gets the advisory on any later candidate. Urgent wins
/// over important; within one severity the newest fix wins. The client skips
/// malformed records, and release validation rejects them before publishing.
pub(super) fn resolve(raw_json: &serde_json::Value, installed: &str, offered: &str)
    -> Option<UpdateAdvisory> {
    let (installed, offered) = match (Version::parse(installed), Version::parse(offered)) {
        (Ok(installed), Ok(offered)) => (installed, offered),
        _ => return None,
    };
    let mut best: Option<(u8, Version, ReleaseAdvisory)> = None;
    for entry in raw_json.get("asb_advisories")?.as_array()? {
        if !entry.is_object() {
            continue;
        }
        let Ok(record) = serde_json::from_value::<ReleaseAdvisory>(entry.clone()) else { continue; };
        let severity = match record.severity.as_str() {
            "important" => Severity::Important,
            "urgent" => Severity::Urgent,
            _ => continue,
        };
        let (Some(affected_from), Some(fixed_in)) =
            (parse_stable_version(&record.affected_from), parse_stable_version(&record.fixed_in))
        else {
            continue;
        };
        if record.reason_zh.trim().is_empty() || record.reason_en.trim().is_empty() {
            continue;
        }
        if !(affected_from <= installed && installed < fixed_in && fixed_in <= offered) {
            continue;
        }
        let rank = severity.rank();
        if best.as_ref().is_some_and(|(known_rank, known_fixed_in, _)| {
            (*known_rank, known_fixed_in) >= (rank, &fixed_in)
        }) {
            continue;
        }
        best = Some((rank, fixed_in, record));
    }
    Some(match best {
        None => UpdateAdvisory::Routine,
        Some((rank, _, record)) if rank == Severity::Urgent.rank() =>
            UpdateAdvisory::Urgent { reason_zh: record.reason_zh, reason_en: record.reason_en },
        Some((_, _, record)) =>
            UpdateAdvisory::Important { reason_zh: record.reason_zh, reason_en: record.reason_en },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn advisory(severity: &str, affected_from: &str, fixed_in: &str) -> serde_json::Value {
        json!({ "severity": severity, "affected_from": affected_from, "fixed_in": fixed_in,
            "reason_zh": "原因", "reason_en": "reason" })
    }

    fn reasons(advisory: &UpdateAdvisory) -> (&str, &str) {
        match advisory {
            UpdateAdvisory::Important { reason_zh, reason_en } | UpdateAdvisory::Urgent { reason_zh, reason_en }
                => (reason_zh, reason_en),
            UpdateAdvisory::Routine => panic!("routine carries no reasons"),
        }
    }

    #[test]
    fn an_affected_install_is_advised_on_any_candidate_containing_the_fix() {
        let manifest = json!({ "asb_advisories": [advisory("urgent", "0.3.10", "0.3.13")] });
        assert!(matches!(resolve(&manifest, "0.3.12", "0.3.15"), Some(UpdateAdvisory::Urgent { .. })));
        assert_eq!(reasons(resolve(&manifest, "0.3.12", "0.3.15").as_ref().unwrap()), ("原因", "reason"));
    }

    #[test]
    fn installs_outside_the_range_and_already_fixed_installs_are_routine() {
        let manifest = json!({ "asb_advisories": [advisory("urgent", "0.3.10", "0.3.13")] });
        for installed in ["0.3.9", "0.3.13", "0.4.0"] {
            assert!(matches!(resolve(&manifest, installed, "0.3.15"), Some(UpdateAdvisory::Routine)));
        }
    }

    #[test]
    fn a_candidate_without_the_fix_downgrades_that_update_to_routine() {
        let manifest = json!({ "asb_advisories": [advisory("urgent", "0.3.10", "0.3.13")] });
        assert!(matches!(resolve(&manifest, "0.3.12", "0.3.12"), Some(UpdateAdvisory::Routine)));
    }

    #[test]
    fn urgent_wins_over_important_and_the_newest_fix_wins_within_a_severity() {
        let manifest = json!({ "asb_advisories": [
            advisory("urgent", "0.3.0", "0.3.4"),
            advisory("important", "0.3.2", "0.3.8"),
        ] });
        assert!(matches!(resolve(&manifest, "0.3.3", "0.3.9"), Some(UpdateAdvisory::Urgent { .. })));

        let important = json!({ "asb_advisories": [
            json!({ "severity": "important", "affected_from": "0.3.0", "fixed_in": "0.3.4",
                "reason_zh": "旧", "reason_en": "old" }),
            json!({ "severity": "important", "affected_from": "0.3.2", "fixed_in": "0.3.8",
                "reason_zh": "新", "reason_en": "new" }),
        ] });
        assert_eq!(reasons(resolve(&important, "0.3.3", "0.3.9").as_ref().unwrap()), ("新", "new"));
    }

    #[test]
    fn malformed_records_and_older_manifests_stay_silent() {
        let malformed = json!({ "asb_advisories": [
            advisory("critical", "0.3.0", "0.3.4"),
            advisory("important", "0.3.6", "0.3.2"),
            advisory("urgent", "0.3.0", "not-a-version"),
            advisory("urgent", "0.03.0", "0.3.4"),
            advisory("urgent", "0.3.0", "0.3.4-beta.1"),
            advisory("urgent", "0.3.0", "0.3.4+build"),
            json!(["urgent", "0.3.0", "0.3.4", "原因", "reason"]),
            json!({ "severity": {"urgent": null}, "affected_from": "0.3.0", "fixed_in": "0.3.4",
                "reason_zh": "原因", "reason_en": "reason" }),
            json!({ "severity": "urgent", "affected_from": ["0.3.0"], "fixed_in": "0.3.4",
                "reason_zh": "原因", "reason_en": "reason" }),
            json!({ "severity": "urgent", "affected_from": "0.3.0", "fixed_in": "0.3.4",
                "reason_zh": "\u{0085}", "reason_en": "reason" }),
            json!({ "severity": "urgent", "affected_from": "0.3.0", "fixed_in": "0.3.4",
                "reason_zh": "原因", "reason_en": "reason", "extra": true }),
            json!({ "severity": "urgent" }),
        ] });
        // The array is present, so the publisher did speak: nothing matched.
        assert!(matches!(resolve(&malformed, "0.3.3", "0.3.9"), Some(UpdateAdvisory::Routine)));

        // Older manifests without a usable field say nothing at all.
        assert!(resolve(&json!({}), "0.3.3", "0.3.9").is_none());
        assert!(resolve(&json!({ "asb_advisories": "nope" }), "0.3.3", "0.3.9").is_none());
    }
}
