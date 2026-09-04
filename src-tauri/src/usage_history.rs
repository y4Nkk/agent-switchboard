//! Credential-free historical usage ledger.
//!
//! This module is the only owner of the persisted history schema. It records
//! normalized values after successful live reads, never receives a renderer
//! path, and never persists provider credentials, endpoints, raw responses,
//! account identifiers, or query source.

use crate::local_state::LocalState;
use asb_core::contracts::{
    CodexOfficialQuota, ProviderProfile, UsageHistoryMetric, UsageHistoryPoint, UsageHistorySeries,
    UsageSummary,
};
use asb_switch::sha256_hex;
use chrono::{DateTime, Duration, FixedOffset, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::sync::{Mutex, OnceLock};
use uuid::Uuid;

const HISTORY_RETENTION_DAYS: i64 = 365;
const MAX_POINTS_PER_SERIES: usize = 720;

static LEDGER_MUTATION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn mutation_lock() -> &'static Mutex<()> {
    LEDGER_MUTATION_LOCK.get_or_init(|| Mutex::new(()))
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UsageHistoryLedger {
    providers: Vec<ProviderHistoryPoint>,
    official: Vec<OfficialHistoryPoint>,
}

impl UsageHistoryLedger {
    fn is_empty(&self) -> bool {
        self.providers.is_empty() && self.official.is_empty()
    }

    fn validate(&self) -> Result<(), String> {
        for point in &self.providers {
            point.validate()?;
        }
        for point in &self.official {
            point.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProviderHistoryPoint {
    profile_id: String,
    query_digest: String,
    at: String,
    plan_name: Option<String>,
    unit: Option<String>,
    remaining: Option<f64>,
    used: Option<f64>,
    total: Option<f64>,
}

impl ProviderHistoryPoint {
    fn validate(&self) -> Result<(), String> {
        if self.profile_id.is_empty() || self.query_digest.is_empty() {
            return Err("供应商历史标识无效".to_string());
        }
        parse_timestamp(&self.at)?;
        validate_optional_number(self.remaining)?;
        validate_optional_number(self.used)?;
        validate_optional_number(self.total)?;
        if self.remaining.is_none() && self.used.is_none() && self.total.is_none() {
            return Err("供应商历史读数为空".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OfficialHistoryPoint {
    at: String,
    window_label: String,
    used_percent: f64,
    resets_at: Option<String>,
}

impl OfficialHistoryPoint {
    fn validate(&self) -> Result<(), String> {
        parse_timestamp(&self.at)?;
        if self.window_label.trim().is_empty()
            || !self.used_percent.is_finite()
            || !(0.0..=100.0).contains(&self.used_percent)
        {
            return Err("官方额度历史窗口无效".to_string());
        }
        if let Some(resets_at) = &self.resets_at {
            parse_timestamp(resets_at)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ProviderSeriesKey {
    plan_name: Option<String>,
    unit: Option<String>,
    metric: UsageHistoryMetric,
}

fn parse_timestamp(value: &str) -> Result<DateTime<FixedOffset>, String> {
    DateTime::parse_from_rfc3339(value).map_err(|_| "历史读取时间无效".to_string())
}

fn normalized_timestamp(value: &str) -> Result<String, String> {
    Ok(parse_timestamp(value)?
        .with_timezone(&Utc)
        .to_rfc3339_opts(SecondsFormat::Millis, true))
}

fn validate_optional_number(value: Option<f64>) -> Result<(), String> {
    if value.is_some_and(|value| !value.is_finite()) {
        return Err("供应商历史数值无效".to_string());
    }
    Ok(())
}

fn normalize_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// Records every normalized reading in one successful provider network query.
/// The caller treats persistence errors as warnings so a fresh query result
/// remains visible even when this optional local history cannot be updated.
pub(crate) fn record_provider(
    state: &LocalState,
    profile: &ProviderProfile,
    summary: &UsageSummary,
) -> Result<(), String> {
    let query = profile
        .usage_query
        .as_ref()
        .ok_or_else(|| "该供应商尚未配置用量查询".to_string())?;
    let digest = crate::usage_cache::query_digest(query)?;
    let at = normalized_timestamp(&summary.at)?;
    let points = summary
        .readings
        .iter()
        .map(|reading| {
            validate_optional_number(reading.remaining)?;
            validate_optional_number(reading.used)?;
            validate_optional_number(reading.total)?;
            if reading.remaining.is_none() && reading.used.is_none() && reading.total.is_none() {
                return Err("供应商用量读数为空".to_string());
            }
            Ok(ProviderHistoryPoint {
                profile_id: profile.id.clone(),
                query_digest: digest.clone(),
                at: at.clone(),
                plan_name: normalize_text(reading.plan_name.as_deref()),
                unit: normalize_text(reading.unit.as_deref()),
                remaining: reading.remaining,
                used: reading.used,
                total: reading.total,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    mutate(state, move |ledger| {
        ledger.providers.extend(points);
        prune_providers(&mut ledger.providers);
    })
}

/// Records every server-declared official quota window from a successful
/// official read. `account_changed` comes from the existing comparison
/// baseline and prevents points belonging to two detected accounts from
/// sharing one trend.
pub(crate) fn record_official(
    state: &LocalState,
    quota: &CodexOfficialQuota,
    reset_history: bool,
) -> Result<(), String> {
    let at = quota
        .at
        .as_deref()
        .ok_or_else(|| "官方额度读取时间缺失".to_string())
        .and_then(normalized_timestamp)?;
    let points = quota
        .windows
        .iter()
        .map(|window| {
            let point = OfficialHistoryPoint {
                at: at.clone(),
                window_label: window.label.trim().to_string(),
                used_percent: window.used_percent,
                resets_at: window
                    .resets_at
                    .as_deref()
                    .map(normalized_timestamp)
                    .transpose()?,
            };
            point.validate()?;
            Ok(point)
        })
        .collect::<Result<Vec<_>, String>>()?;

    mutate(state, move |ledger| {
        if reset_history {
            ledger.official.clear();
        }
        ledger.official.extend(points);
        prune_official(&mut ledger.official);
    })
}

/// Drops every historical provider reading for one profile after an edit or
/// deletion. Official quota history is account-scoped and remains separate.
pub(crate) fn invalidate_provider(state: &LocalState, profile_id: &str) -> Result<(), String> {
    mutate(state, |ledger| {
        ledger
            .providers
            .retain(|point| point.profile_id != profile_id);
    })
}

/// Drops all provider-query history when the application-owned profile store
/// is reset. It deliberately does not alter the independently logged-in
/// Codex official-account trend.
pub(crate) fn clear_providers(state: &LocalState) -> Result<(), String> {
    mutate(state, |ledger| ledger.providers.clear())
}

/// Resolves provider history only for the current profile and its current
/// query digest. Old query shapes cannot surface as a current trend.
pub(crate) fn provider_series(
    state: &LocalState,
    profile: &ProviderProfile,
) -> Result<Vec<UsageHistorySeries>, String> {
    let Some(query) = profile.usage_query.as_ref() else {
        return Ok(Vec::new());
    };
    let digest = crate::usage_cache::query_digest(query)?;
    let Some(ledger) = load(state)? else {
        return Ok(Vec::new());
    };
    let points = ledger
        .providers
        .into_iter()
        .filter(|point| {
            point.profile_id == profile.id
                && point.query_digest == digest
                && is_within_retention(&point.at)
        })
        .collect::<Vec<_>>();
    Ok(provider_series_from_points(&profile.id, &digest, points))
}

/// Returns account-safe official quota series. Account markers are only used
/// by the reset baseline; they never enter this ledger or the renderer data.
pub(crate) fn official_series(state: &LocalState) -> Result<Vec<UsageHistorySeries>, String> {
    let Some(ledger) = load(state)? else {
        return Ok(Vec::new());
    };
    let mut grouped = BTreeMap::<String, Vec<UsageHistoryPoint>>::new();
    for point in ledger
        .official
        .into_iter()
        .filter(|point| is_within_retention(&point.at))
    {
        grouped
            .entry(point.window_label)
            .or_default()
            .push(UsageHistoryPoint {
                at: point.at,
                value: point.used_percent,
            });
    }
    Ok(grouped
        .into_iter()
        .map(|(label, mut points)| {
            sort_points(&mut points);
            UsageHistorySeries {
                id: series_id("official", &[&label]),
                label,
                unit: Some("%".to_string()),
                metric: UsageHistoryMetric::UsedPercent,
                points,
            }
        })
        .collect())
}

fn provider_series_from_points(
    profile_id: &str,
    digest: &str,
    history: Vec<ProviderHistoryPoint>,
) -> Vec<UsageHistorySeries> {
    let mut grouped = BTreeMap::<ProviderSeriesKey, Vec<UsageHistoryPoint>>::new();
    for point in history {
        let used = point
            .used
            .or_else(|| derived_used(point.total, point.remaining));
        let percentage = used.and_then(|used| used_percent(used, point.total));
        push_provider_series_point(
            &mut grouped,
            &point,
            UsageHistoryMetric::Remaining,
            point.remaining,
        );
        push_provider_series_point(&mut grouped, &point, UsageHistoryMetric::Used, used);
        push_provider_series_point(
            &mut grouped,
            &point,
            UsageHistoryMetric::UsedPercent,
            percentage,
        );
    }

    grouped
        .into_iter()
        .map(|(key, mut points)| {
            sort_points(&mut points);
            let plan_label = key.plan_name.as_deref().unwrap_or("默认方案");
            UsageHistorySeries {
                id: series_id(
                    "provider",
                    &[
                        profile_id,
                        digest,
                        plan_label,
                        key.unit.as_deref().unwrap_or(""),
                        metric_key(key.metric),
                    ],
                ),
                label: provider_series_label(plan_label, key.metric),
                unit: match key.metric {
                    UsageHistoryMetric::UsedPercent => Some("%".to_string()),
                    UsageHistoryMetric::Remaining | UsageHistoryMetric::Used => key.unit,
                },
                metric: key.metric,
                points,
            }
        })
        .collect()
}

fn push_provider_series_point(
    grouped: &mut BTreeMap<ProviderSeriesKey, Vec<UsageHistoryPoint>>,
    point: &ProviderHistoryPoint,
    metric: UsageHistoryMetric,
    value: Option<f64>,
) {
    let Some(value) = value.filter(|value| value.is_finite()) else {
        return;
    };
    grouped
        .entry(ProviderSeriesKey {
            plan_name: point.plan_name.clone(),
            unit: point.unit.clone(),
            metric,
        })
        .or_default()
        .push(UsageHistoryPoint {
            at: point.at.clone(),
            value,
        });
}

fn derived_used(total: Option<f64>, remaining: Option<f64>) -> Option<f64> {
    let (total, remaining) = total.zip(remaining)?;
    (total >= remaining).then_some(total - remaining)
}

fn used_percent(used: f64, total: Option<f64>) -> Option<f64> {
    let total = total?;
    (total > 0.0)
        .then_some(used / total * 100.0)
        .filter(|value| value.is_finite())
}

fn metric_key(metric: UsageHistoryMetric) -> &'static str {
    match metric {
        UsageHistoryMetric::Remaining => "remaining",
        UsageHistoryMetric::Used => "used",
        UsageHistoryMetric::UsedPercent => "usedPercent",
    }
}

fn provider_series_label(plan_name: &str, metric: UsageHistoryMetric) -> String {
    let suffix = match metric {
        UsageHistoryMetric::Remaining => "余额",
        UsageHistoryMetric::Used => "已用",
        UsageHistoryMetric::UsedPercent => "已用比例",
    };
    format!("{plan_name}{suffix}")
}

fn series_id(scope: &str, values: &[&str]) -> String {
    let joined = values.join("\u{1f}");
    format!("{scope}-{}", &sha256_hex(&joined)[..16])
}

fn sort_points(points: &mut [UsageHistoryPoint]) {
    points.sort_by(|left, right| left.at.cmp(&right.at));
}

fn is_within_retention(value: &str) -> bool {
    parse_timestamp(value)
        .map(|at| at.with_timezone(&Utc) >= Utc::now() - Duration::days(HISTORY_RETENTION_DAYS))
        .unwrap_or(false)
}

fn prune_providers(points: &mut Vec<ProviderHistoryPoint>) {
    points.retain(|point| is_within_retention(&point.at));
    points.sort_by(|left, right| left.at.cmp(&right.at));
    let mut seen = BTreeMap::<(String, String, Option<String>, Option<String>), usize>::new();
    points.reverse();
    points.retain(|point| {
        let key = (
            point.profile_id.clone(),
            point.query_digest.clone(),
            point.plan_name.clone(),
            point.unit.clone(),
        );
        let count = seen.entry(key).or_default();
        *count += 1;
        *count <= MAX_POINTS_PER_SERIES
    });
    points.reverse();
}

fn prune_official(points: &mut Vec<OfficialHistoryPoint>) {
    points.retain(|point| is_within_retention(&point.at));
    points.sort_by(|left, right| left.at.cmp(&right.at));
    let mut seen = BTreeMap::<String, usize>::new();
    points.reverse();
    points.retain(|point| {
        let count = seen.entry(point.window_label.clone()).or_default();
        *count += 1;
        *count <= MAX_POINTS_PER_SERIES
    });
    points.reverse();
}

fn load(state: &LocalState) -> Result<Option<UsageHistoryLedger>, String> {
    let path = state.usage_history_path();
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("用量历史不可读".to_string()),
    };
    let ledger: UsageHistoryLedger =
        serde_json::from_str(&text).map_err(|_| "用量历史格式无效".to_string())?;
    ledger
        .validate()
        .map_err(|_| "用量历史格式无效".to_string())?;
    Ok(Some(ledger))
}

fn mutate(
    state: &LocalState,
    operation: impl FnOnce(&mut UsageHistoryLedger),
) -> Result<(), String> {
    let _guard = mutation_lock()
        .lock()
        .map_err(|_| "用量历史写入锁不可用".to_string())?;
    let mut ledger = load(state)?.unwrap_or_default();
    operation(&mut ledger);
    if ledger.is_empty() {
        return match fs::remove_file(state.usage_history_path()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err("无法清除用量历史".to_string()),
        };
    }
    save(state, &ledger)
}

fn save(state: &LocalState, ledger: &UsageHistoryLedger) -> Result<(), String> {
    let content =
        serde_json::to_string_pretty(ledger).map_err(|_| "用量历史序列化失败".to_string())?;
    let path = state.usage_history_path();
    let parent = path
        .parent()
        .ok_or_else(|| "用量历史目录无效".to_string())?;
    fs::create_dir_all(parent).map_err(|_| "无法创建应用数据目录".to_string())?;
    let temporary = parent.join(format!("usage-history.{}.tmp", Uuid::new_v4()));
    fs::write(&temporary, content).map_err(|_| "无法写入用量历史临时文件".to_string())?;
    if fs::rename(&temporary, path).is_err() {
        let _ = fs::remove_file(&temporary);
        return Err("无法原子保存用量历史".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use asb_core::contracts::{
        AppKind, CodexOfficialQuotaStatus, CodexOfficialQuotaWindow, ProviderDraft, RouteMode,
        UsageQuery, UsageReading,
    };
    use tempfile::tempdir;

    fn provider() -> ProviderProfile {
        ProviderProfile::from_draft(
            "provider-1".to_string(),
            ProviderDraft {
                app: AppKind::Codex,
                route_mode: RouteMode::Custom,
                name: "示例中转".to_string(),
                model: None,
                base_url: Some("https://relay.example".to_string()),
                api_key: "test-api-key".to_string(),
                model_options: None,
                notes: None,
                website_url: None,
                usage_query: Some(UsageQuery::Script {
                    source: "({ request() {}, extract() {} })".to_string(),
                    refresh_interval_minutes: 0,
                }),
            },
        )
    }

    fn summary(at: &str) -> UsageSummary {
        UsageSummary {
            at: at.to_string(),
            readings: vec![UsageReading {
                plan_name: Some("专业版".to_string()),
                remaining: Some(70.0),
                used: None,
                total: Some(100.0),
                unit: Some("次".to_string()),
            }],
        }
    }

    fn quota(at: &str, used_percent: f64) -> CodexOfficialQuota {
        CodexOfficialQuota {
            status: CodexOfficialQuotaStatus::Available,
            windows: vec![CodexOfficialQuotaWindow {
                label: "7 天".to_string(),
                used_percent,
                resets_at: Some("2026-09-10T00:00:00Z".to_string()),
            }],
            at: Some(at.to_string()),
            stale: false,
            last_reset: None,
        }
    }

    #[test]
    fn records_provider_numbers_without_persisting_profile_secrets_or_query_source() {
        let directory = tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        let profile = provider();

        record_provider(&state, &profile, &summary("2026-09-03T08:00:00Z"))
            .expect("record provider history");

        let persisted = fs::read_to_string(state.usage_history_path()).expect("history file");
        assert!(!persisted.contains("test-api-key"));
        assert!(!persisted.contains("relay.example"));
        assert!(!persisted.contains("extract()"));

        let series = provider_series(&state, &profile).expect("provider series");
        assert_eq!(series.len(), 3);
        assert!(series.iter().any(|series| {
            series.metric == UsageHistoryMetric::Remaining
                && series.points[0].value == 70.0
                && series.unit.as_deref() == Some("次")
        }));
        assert!(series.iter().any(|series| {
            series.metric == UsageHistoryMetric::Used && series.points[0].value == 30.0
        }));
        assert!(series.iter().any(|series| {
            series.metric == UsageHistoryMetric::UsedPercent
                && series.points[0].value == 30.0
                && series.unit.as_deref() == Some("%")
        }));
    }

    #[test]
    fn successive_provider_reads_append_to_the_same_trend() {
        let directory = tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        let profile = provider();

        record_provider(&state, &profile, &summary("2026-09-03T08:00:00Z"))
            .expect("record first provider history");
        record_provider(&state, &profile, &summary("2026-09-03T09:00:00Z"))
            .expect("record second provider history");

        let remaining = provider_series(&state, &profile)
            .expect("provider series")
            .into_iter()
            .find(|series| series.metric == UsageHistoryMetric::Remaining)
            .expect("remaining series");
        assert_eq!(remaining.points.len(), 2);
        assert_eq!(remaining.points[1].at, "2026-09-03T09:00:00.000Z");
    }

    #[test]
    fn only_the_current_provider_query_digest_can_read_its_history() {
        let directory = tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        let profile = provider();
        record_provider(&state, &profile, &summary("2026-09-03T08:00:00Z"))
            .expect("record provider history");
        let mut changed = profile.clone();
        changed.usage_query = Some(UsageQuery::Script {
            source: "({ request() { return {}; }, extract() { return {}; } })".to_string(),
            refresh_interval_minutes: 0,
        });

        assert!(provider_series(&state, &changed)
            .expect("changed query series")
            .is_empty());
    }

    #[test]
    fn invalidating_one_profile_and_resetting_the_store_remove_provider_history() {
        let directory = tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        let profile = provider();
        record_provider(&state, &profile, &summary("2026-09-03T08:00:00Z"))
            .expect("record provider history");

        invalidate_provider(&state, &profile.id).expect("invalidate profile history");
        assert!(provider_series(&state, &profile)
            .expect("invalidated profile series")
            .is_empty());
        assert!(!state.usage_history_path().exists());

        record_provider(&state, &profile, &summary("2026-09-03T09:00:00Z"))
            .expect("record provider history again");
        record_official(&state, &quota("2026-09-03T09:00:00Z", 20.0), false)
            .expect("record independent official history");

        clear_providers(&state).expect("reset provider history");

        assert!(provider_series(&state, &profile)
            .expect("cleared provider series")
            .is_empty());
        assert_eq!(
            official_series(&state)
                .expect("official history remains")
                .len(),
            1
        );
    }

    #[test]
    fn pruning_drops_points_older_than_the_365_day_retention_window() {
        let recent = Utc::now().to_rfc3339();
        let expired = (Utc::now() - Duration::days(HISTORY_RETENTION_DAYS + 1)).to_rfc3339();
        let mut points = vec![
            ProviderHistoryPoint {
                profile_id: "profile".to_string(),
                query_digest: "digest".to_string(),
                at: expired,
                plan_name: None,
                unit: None,
                remaining: Some(1.0),
                used: None,
                total: None,
            },
            ProviderHistoryPoint {
                profile_id: "profile".to_string(),
                query_digest: "digest".to_string(),
                at: recent,
                plan_name: None,
                unit: None,
                remaining: Some(2.0),
                used: None,
                total: None,
            },
        ];

        prune_providers(&mut points);

        assert_eq!(points.len(), 1);
        assert_eq!(points[0].remaining, Some(2.0));
    }

    #[test]
    fn reading_history_hides_expired_snapshots_without_a_new_successful_read() {
        let directory = tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        let profile = provider();
        let digest =
            crate::usage_cache::query_digest(profile.usage_query.as_ref().expect("usage query"))
                .expect("query digest");
        let expired = (Utc::now() - Duration::days(HISTORY_RETENTION_DAYS + 1)).to_rfc3339();
        let ledger = UsageHistoryLedger {
            providers: vec![ProviderHistoryPoint {
                profile_id: profile.id.clone(),
                query_digest: digest,
                at: expired.clone(),
                plan_name: Some("专业版".to_string()),
                unit: Some("次".to_string()),
                remaining: Some(70.0),
                used: None,
                total: Some(100.0),
            }],
            official: vec![OfficialHistoryPoint {
                at: expired,
                window_label: "7 天".to_string(),
                used_percent: 20.0,
                resets_at: None,
            }],
        };
        save(&state, &ledger).expect("persist expired history");

        assert!(provider_series(&state, &profile)
            .expect("provider history")
            .is_empty());
        assert!(official_series(&state)
            .expect("official history")
            .is_empty());
    }

    #[test]
    fn detected_official_account_change_replaces_the_official_trend() {
        let directory = tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        record_official(&state, &quota("2026-09-03T08:00:00Z", 20.0), false)
            .expect("record first account");
        record_official(&state, &quota("2026-09-03T09:00:00Z", 40.0), true)
            .expect("record changed account");

        let series = official_series(&state).expect("official series");
        assert_eq!(series.len(), 1);
        assert_eq!(
            series[0].points,
            vec![UsageHistoryPoint {
                at: "2026-09-03T09:00:00.000Z".to_string(),
                value: 40.0,
            }]
        );
    }

    #[test]
    fn malformed_history_is_rejected_without_rewriting_it() {
        let directory = tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        fs::create_dir_all(state.usage_history_path().parent().expect("parent"))
            .expect("state directory");
        let malformed = r#"{"providers":[{"profileId":"p"}],"official":[]}"#;
        fs::write(state.usage_history_path(), malformed).expect("malformed history");

        assert_eq!(
            record_provider(&state, &provider(), &summary("2026-09-03T08:00:00Z")).unwrap_err(),
            "用量历史格式无效"
        );
        assert_eq!(
            fs::read_to_string(state.usage_history_path()).expect("original history"),
            malformed
        );
    }

    #[test]
    fn pruning_keeps_the_newest_720_points_in_each_provider_series() {
        let recent = Utc::now();
        let mut points = (0..722)
            .map(|index| ProviderHistoryPoint {
                profile_id: "profile".to_string(),
                query_digest: "digest".to_string(),
                at: (recent - Duration::milliseconds((721 - index) as i64)).to_rfc3339(),
                plan_name: None,
                unit: None,
                remaining: Some(index as f64),
                used: None,
                total: None,
            })
            .collect::<Vec<_>>();

        prune_providers(&mut points);

        assert_eq!(points.len(), MAX_POINTS_PER_SERIES);
        assert_eq!(points[0].remaining, Some(2.0));
        assert_eq!(points.last().and_then(|point| point.remaining), Some(721.0));
    }
}
