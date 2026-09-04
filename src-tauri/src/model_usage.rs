//! Read-only aggregation of token usage recorded in local client sessions.
//!
//! Local records establish observed token consumption, never a provider
//! balance or remaining subscription allowance. This module does not create,
//! modify, or index a session file or SQLite database.

use asb_core::contracts::{
    AppKind, ModelUsageDay, ModelUsageGroup, ModelUsageIssue, ModelUsageRange, ModelUsageReport,
    ModelUsageTokens,
};
use chrono::{DateTime, Duration, Local, NaiveDate, Utc};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct TokenTotals {
    input: u64,
    cache_read: u64,
    cache_creation: u64,
    output: u64,
    total: u64,
}

impl TokenTotals {
    fn delta_from(self, previous: Option<Self>) -> Option<Self> {
        let Some(previous) = previous else {
            return Some(self);
        };
        Some(Self {
            input: self.input.checked_sub(previous.input)?,
            cache_read: self.cache_read.checked_sub(previous.cache_read)?,
            cache_creation: self.cache_creation.checked_sub(previous.cache_creation)?,
            output: self.output.checked_sub(previous.output)?,
            total: self.total.checked_sub(previous.total)?,
        })
    }

    fn checked_add(self, other: Self) -> Option<Self> {
        Some(Self {
            input: self.input.checked_add(other.input)?,
            cache_read: self.cache_read.checked_add(other.cache_read)?,
            cache_creation: self.cache_creation.checked_add(other.cache_creation)?,
            output: self.output.checked_add(other.output)?,
            total: self.total.checked_add(other.total)?,
        })
    }

    fn into_contract(self) -> ModelUsageTokens {
        ModelUsageTokens {
            input_tokens: self.input,
            cache_read_input_tokens: self.cache_read,
            cache_creation_input_tokens: self.cache_creation,
            output_tokens: self.output,
            total_tokens: self.total,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct GroupKey {
    app: AppKind,
    model: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
struct GroupTotals {
    tokens: TokenTotals,
    session_count: u64,
}

#[derive(Debug, Clone)]
struct ClaudeUsageCandidate {
    model: String,
    tokens: TokenTotals,
    timestamp: Option<String>,
    has_stop_reason: bool,
}

#[derive(Debug, Clone, Copy)]
struct CalendarRange {
    start: Option<NaiveDate>,
    end: NaiveDate,
}

impl CalendarRange {
    fn new(range: ModelUsageRange, now: DateTime<Local>) -> Self {
        let end = now.date_naive();
        let start = match range {
            ModelUsageRange::Today => Some(end),
            ModelUsageRange::Last7Days => Some(end - Duration::days(6)),
            ModelUsageRange::Last30Days => Some(end - Duration::days(29)),
            ModelUsageRange::All => None,
        };
        Self { start, end }
    }

    fn assignment(self, timestamp: Option<&str>) -> Option<Option<NaiveDate>> {
        match timestamp.and_then(local_date) {
            Some(date)
                if self
                    .start
                    .is_none_or(|start| (start..=self.end).contains(&date)) =>
            {
                Some(Some(date))
            }
            Some(_) => None,
            None if self.start.is_none() => Some(None),
            None => None,
        }
    }
}

/// Builds one report from the fixed user-level session roots. Failures at one
/// root or file become local issues; readable records from other sources stay
/// available to the renderer.
pub(crate) fn get_model_usage_report(range: ModelUsageRange) -> ModelUsageReport {
    let now = Local::now();
    let roots = match crate::session_manager::session_roots() {
        Ok(roots) => roots,
        Err(message) => {
            return ModelUsageReport {
                range,
                generated_at: now.with_timezone(&Utc).to_rfc3339(),
                groups: Vec::new(),
                days: Vec::new(),
                unassigned_tokens: ModelUsageTokens::default(),
                issues: [AppKind::Codex, AppKind::Claude]
                    .into_iter()
                    .map(|app| ModelUsageIssue {
                        app,
                        message: message.clone(),
                    })
                    .collect(),
            }
        }
    };
    report_from_roots(range, &roots, now)
}

fn report_from_roots(
    range: ModelUsageRange,
    roots: &[(AppKind, std::path::PathBuf)],
    now: DateTime<Local>,
) -> ModelUsageReport {
    let calendar_range = CalendarRange::new(range, now);
    let mut groups = BTreeMap::new();
    let mut days = BTreeMap::new();
    let mut unassigned_tokens = TokenTotals::default();
    let mut issues = Vec::new();
    let mut seen_sessions = HashSet::new();

    for (app, root) in roots {
        match crate::session_manager::collect_session_jsonl_files(root) {
            Ok(paths) => {
                for path in paths {
                    match crate::session_manager::session_id_in_session_file(&path) {
                        Ok(Some(session_id)) => {
                            if !seen_sessions.insert((*app, session_id)) {
                                continue;
                            }
                            scan_session_file(
                                *app,
                                &path,
                                calendar_range,
                                &mut groups,
                                &mut days,
                                &mut unassigned_tokens,
                                &mut issues,
                            );
                        }
                        Ok(None) => scan_session_file(
                            *app,
                            &path,
                            calendar_range,
                            &mut groups,
                            &mut days,
                            &mut unassigned_tokens,
                            &mut issues,
                        ),
                        Err(_) => issues.push(ModelUsageIssue {
                            app: *app,
                            message: format!("无法读取{}会话记录", client_label(*app)),
                        }),
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => issues.push(ModelUsageIssue {
                app: *app,
                message: format!("无法读取{}会话目录", client_label(*app)),
            }),
        }
    }

    ModelUsageReport {
        range,
        generated_at: now.with_timezone(&Utc).to_rfc3339(),
        groups: groups
            .into_iter()
            .map(|(key, totals)| ModelUsageGroup {
                app: key.app,
                model: key.model,
                input_tokens: totals.tokens.input,
                cache_read_input_tokens: totals.tokens.cache_read,
                cache_creation_input_tokens: totals.tokens.cache_creation,
                output_tokens: totals.tokens.output,
                total_tokens: totals.tokens.total,
                session_count: totals.session_count,
            })
            .collect(),
        days: days
            .into_iter()
            .map(|(date, totals)| ModelUsageDay {
                date: date.format("%F").to_string(),
                input_tokens: totals.input,
                cache_read_input_tokens: totals.cache_read,
                cache_creation_input_tokens: totals.cache_creation,
                output_tokens: totals.output,
                total_tokens: totals.total,
            })
            .collect(),
        unassigned_tokens: unassigned_tokens.into_contract(),
        issues: unique_issues(issues),
    }
}

fn scan_session_file(
    app: AppKind,
    path: &Path,
    calendar_range: CalendarRange,
    groups: &mut BTreeMap<GroupKey, GroupTotals>,
    days: &mut BTreeMap<NaiveDate, TokenTotals>,
    unassigned_tokens: &mut TokenTotals,
    issues: &mut Vec<ModelUsageIssue>,
) {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(_) => {
            issues.push(ModelUsageIssue {
                app,
                message: format!("无法读取{}会话记录", client_label(app)),
            });
            return;
        }
    };
    let mut file_groups = BTreeSet::new();
    match app {
        AppKind::Codex => scan_codex_session(
            BufReader::new(file),
            calendar_range,
            groups,
            days,
            unassigned_tokens,
            &mut file_groups,
            issues,
        ),
        AppKind::Claude => scan_claude_session(
            BufReader::new(file),
            calendar_range,
            groups,
            days,
            unassigned_tokens,
            &mut file_groups,
            issues,
        ),
    }
    for key in file_groups {
        if let Some(totals) = groups.get_mut(&key) {
            totals.session_count = totals.session_count.saturating_add(1);
        }
    }
}

fn scan_codex_session(
    reader: BufReader<File>,
    calendar_range: CalendarRange,
    groups: &mut BTreeMap<GroupKey, GroupTotals>,
    days: &mut BTreeMap<NaiveDate, TokenTotals>,
    unassigned_tokens: &mut TokenTotals,
    file_groups: &mut BTreeSet<GroupKey>,
    issues: &mut Vec<ModelUsageIssue>,
) {
    let mut model = None;
    let mut previous = None;
    let mut reported_corruption = false;

    for line in reader.lines() {
        let line = match line {
            Ok(line) => line,
            Err(_) => {
                issues.push(ModelUsageIssue {
                    app: AppKind::Codex,
                    message: "无法完整读取 Codex 会话记录".to_string(),
                });
                break;
            }
        };
        let value = match serde_json::from_str::<Value>(&line) {
            Ok(value) => value,
            Err(_) => {
                report_corruption(AppKind::Codex, issues, &mut reported_corruption);
                continue;
            }
        };
        if let Some(context_model) = codex_turn_context_model(&value) {
            model = Some(context_model);
            continue;
        }
        let Some(tokens) = codex_token_totals(&value) else {
            continue;
        };
        let delta = tokens.delta_from(previous);
        previous = Some(tokens);
        let Some(delta) = delta else {
            continue;
        };
        let Some(assignment) = calendar_range.assignment(timestamp(&value)) else {
            continue;
        };
        add_tokens(
            groups,
            days,
            unassigned_tokens,
            file_groups,
            assignment,
            GroupKey {
                app: AppKind::Codex,
                model: model.clone(),
            },
            delta,
        );
    }
}

fn scan_claude_session(
    reader: BufReader<File>,
    calendar_range: CalendarRange,
    groups: &mut BTreeMap<GroupKey, GroupTotals>,
    days: &mut BTreeMap<NaiveDate, TokenTotals>,
    unassigned_tokens: &mut TokenTotals,
    file_groups: &mut BTreeSet<GroupKey>,
    issues: &mut Vec<ModelUsageIssue>,
) {
    let mut reported_corruption = false;
    let mut candidates = BTreeMap::new();
    for line in reader.lines() {
        let line = match line {
            Ok(line) => line,
            Err(_) => {
                issues.push(ModelUsageIssue {
                    app: AppKind::Claude,
                    message: "无法完整读取 Claude Code 会话记录".to_string(),
                });
                break;
            }
        };
        let value = match serde_json::from_str::<Value>(&line) {
            Ok(value) => value,
            Err(_) => {
                report_corruption(AppKind::Claude, issues, &mut reported_corruption);
                continue;
            }
        };
        let Some((message_id, candidate)) = claude_usage(&value) else {
            continue;
        };
        match candidates.get(&message_id) {
            Some(current) if !prefer_claude_candidate(&candidate, current) => {}
            _ => {
                candidates.insert(message_id, candidate);
            }
        }
    }

    for candidate in candidates.into_values() {
        if candidate.tokens.total == 0 {
            continue;
        }
        let Some(assignment) = calendar_range.assignment(candidate.timestamp.as_deref()) else {
            continue;
        };
        add_tokens(
            groups,
            days,
            unassigned_tokens,
            file_groups,
            assignment,
            GroupKey {
                app: AppKind::Claude,
                model: Some(candidate.model),
            },
            candidate.tokens,
        );
    }
}

fn add_tokens(
    groups: &mut BTreeMap<GroupKey, GroupTotals>,
    days: &mut BTreeMap<NaiveDate, TokenTotals>,
    unassigned_tokens: &mut TokenTotals,
    file_groups: &mut BTreeSet<GroupKey>,
    assignment: Option<NaiveDate>,
    key: GroupKey,
    tokens: TokenTotals,
) {
    let destination = match assignment {
        Some(date) => days.entry(date).or_default(),
        None => unassigned_tokens,
    };
    let Some(updated_destination) = destination.checked_add(tokens) else {
        return;
    };
    let totals = groups.entry(key.clone()).or_default();
    let Some(updated) = totals.tokens.checked_add(tokens) else {
        return;
    };
    *destination = updated_destination;
    totals.tokens = updated;
    file_groups.insert(key);
}

fn report_corruption(app: AppKind, issues: &mut Vec<ModelUsageIssue>, reported: &mut bool) {
    if *reported {
        return;
    }
    *reported = true;
    issues.push(ModelUsageIssue {
        app,
        message: format!("{}会话记录中存在无法解析的条目", client_label(app)),
    });
}

fn unique_issues(issues: Vec<ModelUsageIssue>) -> Vec<ModelUsageIssue> {
    let mut seen = HashSet::new();
    issues
        .into_iter()
        .filter(|issue| seen.insert((issue.app, issue.message.clone())))
        .collect()
}

fn codex_turn_context_model(value: &Value) -> Option<String> {
    (value.get("type").and_then(Value::as_str) == Some("turn_context"))
        .then(|| value.get("payload")?.get("model")?.as_str())
        .flatten()
        .filter(|model| !model.trim().is_empty())
        .map(str::to_string)
}

fn codex_token_totals(value: &Value) -> Option<TokenTotals> {
    if value.get("type").and_then(Value::as_str) != Some("event_msg")
        || value.get("payload")?.get("type").and_then(Value::as_str) != Some("token_count")
    {
        return None;
    }
    let totals = value
        .get("payload")?
        .get("info")?
        .get("total_token_usage")?;
    let input = token_value(totals, "input_tokens")?;
    let cache_read = token_value(totals, "cached_input_tokens")?;
    let output = token_value(totals, "output_tokens")?;
    // Codex records cached input as a subset of input_tokens. The shared
    // report separates fresh input from the cache-read portion.
    let fresh_input = input.checked_sub(cache_read)?;
    Some(TokenTotals {
        input: fresh_input,
        cache_read,
        cache_creation: 0,
        output,
        total: fresh_input.checked_add(cache_read)?.checked_add(output)?,
    })
}

fn claude_usage(value: &Value) -> Option<(String, ClaudeUsageCandidate)> {
    if value.get("type").and_then(Value::as_str) != Some("assistant") {
        return None;
    }
    let message = value.get("message")?;
    let message_id = message.get("id")?.as_str()?.to_string();
    let model = message
        .get("model")?
        .as_str()
        .filter(|model| !model.trim().is_empty())?
        .to_string();
    let usage = message.get("usage")?;
    let input = token_value(usage, "input_tokens")?;
    let cache_creation = optional_token_value(usage, "cache_creation_input_tokens")?;
    let cache_read = optional_token_value(usage, "cache_read_input_tokens")?;
    let output = token_value(usage, "output_tokens")?;
    let total = input
        .checked_add(cache_creation)?
        .checked_add(cache_read)?
        .checked_add(output)?;
    Some((
        message_id,
        ClaudeUsageCandidate {
            model,
            tokens: TokenTotals {
                input,
                cache_read,
                cache_creation,
                output,
                total,
            },
            timestamp: timestamp(value).map(str::to_string),
            has_stop_reason: message
                .get("stop_reason")
                .and_then(Value::as_str)
                .is_some_and(|reason| !reason.trim().is_empty()),
        },
    ))
}

fn prefer_claude_candidate(
    candidate: &ClaudeUsageCandidate,
    current: &ClaudeUsageCandidate,
) -> bool {
    candidate.has_stop_reason && !current.has_stop_reason
        || candidate.has_stop_reason == current.has_stop_reason
            && candidate.tokens.output >= current.tokens.output
}

fn token_value(value: &Value, key: &str) -> Option<u64> {
    value.get(key)?.as_u64()
}

fn optional_token_value(value: &Value, key: &str) -> Option<u64> {
    value.get(key).map_or(Some(0), Value::as_u64)
}

fn timestamp(value: &Value) -> Option<&str> {
    value.get("timestamp")?.as_str()
}

fn local_date(timestamp: &str) -> Option<NaiveDate> {
    DateTime::parse_from_rfc3339(timestamp)
        .ok()
        .map(|timestamp| timestamp.with_timezone(&Local).date_naive())
}

fn client_label(app: AppKind) -> &'static str {
    match app {
        AppKind::Codex => "Codex",
        AppKind::Claude => "Claude Code",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        fs::write(path, content).expect("write session");
    }

    fn timestamp(days_before_now: i64) -> String {
        (Local::now() - Duration::days(days_before_now)).to_rfc3339()
    }

    fn roots(temp: &tempfile::TempDir) -> Vec<(AppKind, std::path::PathBuf)> {
        vec![
            (AppKind::Codex, temp.path().join("codex")),
            (AppKind::Claude, temp.path().join("claude")),
        ]
    }

    #[test]
    fn aggregates_codex_cumulative_counts_by_turn_context_model() {
        let temp = tempdir().expect("temp");
        let at = timestamp(0);
        write(
            &temp.path().join("codex").join("usage.jsonl"),
            &format!(
                concat!(
                    "{{\"type\":\"turn_context\",\"payload\":{{\"model\":\"gpt-5\"}}}}\n",
                    "{{\"timestamp\":\"{at}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"token_count\",\"info\":{{\"total_token_usage\":{{\"input_tokens\":10,\"cached_input_tokens\":2,\"output_tokens\":3,\"total_tokens\":13}}}}}}}}\n",
                    "{{\"timestamp\":\"{at}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"token_count\",\"info\":{{\"total_token_usage\":{{\"input_tokens\":16,\"cached_input_tokens\":4,\"output_tokens\":5,\"total_tokens\":21}}}}}}}}\n",
                    "{{\"type\":\"turn_context\",\"payload\":{{\"model\":\"gpt-4\"}}}}\n",
                    "{{\"timestamp\":\"{at}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"token_count\",\"info\":{{\"total_token_usage\":{{\"input_tokens\":3,\"cached_input_tokens\":0,\"output_tokens\":1,\"total_tokens\":4}}}}}}}}\n",
                    "{{\"timestamp\":\"{at}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"token_count\",\"info\":{{\"total_token_usage\":{{\"input_tokens\":5,\"cached_input_tokens\":1,\"output_tokens\":2,\"total_tokens\":7}}}}}}}}\n"
                ),
                at = at
            ),
        );

        let report = report_from_roots(ModelUsageRange::Today, &roots(&temp), Local::now());
        assert!(report.issues.is_empty());
        assert_eq!(report.groups.len(), 2);
        assert_eq!(
            report.groups[0],
            ModelUsageGroup {
                app: AppKind::Codex,
                model: Some("gpt-4".to_string()),
                input_tokens: 1,
                cache_read_input_tokens: 1,
                cache_creation_input_tokens: 0,
                output_tokens: 1,
                total_tokens: 3,
                session_count: 1,
            }
        );
        assert_eq!(
            report.groups[1],
            ModelUsageGroup {
                app: AppKind::Codex,
                model: Some("gpt-5".to_string()),
                input_tokens: 12,
                cache_read_input_tokens: 4,
                cache_creation_input_tokens: 0,
                output_tokens: 5,
                total_tokens: 21,
                session_count: 1,
            }
        );
        assert_eq!(
            report.days,
            vec![ModelUsageDay {
                date: local_date(&at)
                    .expect("local date")
                    .format("%F")
                    .to_string(),
                input_tokens: 13,
                cache_read_input_tokens: 5,
                cache_creation_input_tokens: 0,
                output_tokens: 6,
                total_tokens: 24,
            }]
        );
        assert_eq!(report.unassigned_tokens, ModelUsageTokens::default());
    }

    #[test]
    fn counts_only_explicit_claude_usage_and_keeps_partial_results() {
        let temp = tempdir().expect("temp");
        let at = timestamp(0);
        write(
            &temp.path().join("claude").join("usage.jsonl"),
            &format!(
                concat!(
                    "not json\n",
                    "{{\"timestamp\":\"{at}\",\"type\":\"assistant\",\"message\":{{\"id\":\"message-1\",\"model\":\"claude-sonnet\",\"usage\":{{\"input_tokens\":10,\"cache_creation_input_tokens\":3,\"cache_read_input_tokens\":2,\"output_tokens\":4}}}}}}\n",
                    "{{\"timestamp\":\"{at}\",\"type\":\"assistant\",\"message\":{{\"id\":\"message-zero\",\"model\":\"claude-sonnet\",\"usage\":{{\"input_tokens\":0,\"output_tokens\":0}}}}}}\n",
                    "{{\"timestamp\":\"{at}\",\"type\":\"assistant\",\"message\":{{\"usage\":{{\"input_tokens\":99,\"output_tokens\":99}}}}}}\n"
                ),
                at = at
            ),
        );

        let report = report_from_roots(ModelUsageRange::Today, &roots(&temp), Local::now());
        assert_eq!(report.issues.len(), 1);
        assert_eq!(report.issues[0].app, AppKind::Claude);
        assert_eq!(
            report.groups,
            vec![ModelUsageGroup {
                app: AppKind::Claude,
                model: Some("claude-sonnet".to_string()),
                input_tokens: 10,
                cache_read_input_tokens: 2,
                cache_creation_input_tokens: 3,
                output_tokens: 4,
                total_tokens: 19,
                session_count: 1,
            }]
        );
    }

    #[test]
    fn time_ranges_use_the_local_calendar_but_keep_the_cumulative_baseline() {
        let temp = tempdir().expect("temp");
        let old = timestamp(8);
        let current = timestamp(0);
        write(
            &temp.path().join("codex").join("usage.jsonl"),
            &format!(
                concat!(
                    "{{\"type\":\"turn_context\",\"payload\":{{\"model\":\"gpt-5\"}}}}\n",
                    "{{\"timestamp\":\"{old}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"token_count\",\"info\":{{\"total_token_usage\":{{\"input_tokens\":4,\"cached_input_tokens\":1,\"output_tokens\":1,\"total_tokens\":5}}}}}}}}\n",
                    "{{\"timestamp\":\"{current}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"token_count\",\"info\":{{\"total_token_usage\":{{\"input_tokens\":7,\"cached_input_tokens\":2,\"output_tokens\":3,\"total_tokens\":10}}}}}}}}\n"
                ),
                old = old,
                current = current
            ),
        );

        let report = report_from_roots(ModelUsageRange::Last7Days, &roots(&temp), Local::now());
        assert_eq!(report.groups.len(), 1);
        assert_eq!(report.groups[0].input_tokens, 2);
        assert_eq!(report.groups[0].cache_read_input_tokens, 1);
        assert_eq!(report.groups[0].cache_creation_input_tokens, 0);
        assert_eq!(report.groups[0].output_tokens, 2);
        assert_eq!(report.groups[0].total_tokens, 5);
        assert_eq!(
            report.days,
            vec![ModelUsageDay {
                date: local_date(&current)
                    .expect("current local date")
                    .format("%F")
                    .to_string(),
                input_tokens: 2,
                cache_read_input_tokens: 1,
                cache_creation_input_tokens: 0,
                output_tokens: 2,
                total_tokens: 5,
            }]
        );
    }

    #[test]
    fn claude_duplicates_choose_a_completed_message_then_largest_output() {
        let temp = tempdir().expect("temp");
        let at = timestamp(0);
        write(
            &temp.path().join("claude").join("usage.jsonl"),
            &format!(
                concat!(
                    "{{\"timestamp\":\"{at}\",\"type\":\"assistant\",\"message\":{{\"id\":\"message-1\",\"model\":\"claude-sonnet\",\"usage\":{{\"input_tokens\":2,\"output_tokens\":9}}}}}}\n",
                    "{{\"timestamp\":\"{at}\",\"type\":\"assistant\",\"message\":{{\"id\":\"message-1\",\"model\":\"claude-sonnet\",\"stop_reason\":\"end_turn\",\"usage\":{{\"input_tokens\":3,\"cache_creation_input_tokens\":4,\"cache_read_input_tokens\":5,\"output_tokens\":6}}}}}}\n",
                    "{{\"timestamp\":\"{at}\",\"type\":\"assistant\",\"message\":{{\"id\":\"message-1\",\"model\":\"claude-sonnet\",\"stop_reason\":\"end_turn\",\"usage\":{{\"input_tokens\":7,\"cache_creation_input_tokens\":8,\"cache_read_input_tokens\":9,\"output_tokens\":10}}}}}}\n"
                ),
                at = at
            ),
        );

        let report = report_from_roots(ModelUsageRange::Today, &roots(&temp), Local::now());
        assert_eq!(report.groups.len(), 1);
        assert_eq!(report.groups[0].input_tokens, 7);
        assert_eq!(report.groups[0].cache_read_input_tokens, 9);
        assert_eq!(report.groups[0].cache_creation_input_tokens, 8);
        assert_eq!(report.groups[0].output_tokens, 10);
        assert_eq!(report.groups[0].total_tokens, 34);
        assert_eq!(report.groups[0].session_count, 1);
    }

    #[test]
    fn claude_duplicate_uses_the_selected_completion_timestamp_for_its_daily_bucket() {
        let temp = tempdir().expect("temp");
        let previous = timestamp(1);
        let current = timestamp(0);
        write(
            &temp.path().join("claude").join("usage.jsonl"),
            &format!(
                concat!(
                    "{{\"timestamp\":\"{previous}\",\"type\":\"assistant\",\"message\":{{\"id\":\"message-1\",\"model\":\"claude-sonnet\",\"usage\":{{\"input_tokens\":2,\"output_tokens\":3}}}}}}\n",
                    "{{\"timestamp\":\"{current}\",\"type\":\"assistant\",\"message\":{{\"id\":\"message-1\",\"model\":\"claude-sonnet\",\"stop_reason\":\"end_turn\",\"usage\":{{\"input_tokens\":4,\"output_tokens\":5}}}}}}\n"
                ),
                previous = previous,
                current = current,
            ),
        );

        let report = report_from_roots(ModelUsageRange::Last7Days, &roots(&temp), Local::now());

        assert_eq!(report.groups.len(), 1);
        assert_eq!(report.groups[0].total_tokens, 9);
        assert_eq!(
            report.days,
            vec![ModelUsageDay {
                date: local_date(&current)
                    .expect("current local date")
                    .format("%F")
                    .to_string(),
                input_tokens: 4,
                cache_read_input_tokens: 0,
                cache_creation_input_tokens: 0,
                output_tokens: 5,
                total_tokens: 9,
            }]
        );
    }

    #[test]
    fn duplicate_archived_session_keeps_the_first_configured_root() {
        let temp = tempdir().expect("temp");
        let at = timestamp(0);
        let record = |session_id: &str, input: u64| {
            [
                serde_json::json!({"type": "session_meta", "payload": {"id": session_id}}),
                serde_json::json!({"type": "turn_context", "payload": {"model": "gpt-5"}}),
                serde_json::json!({
                    "timestamp": at,
                    "type": "event_msg",
                    "payload": {
                        "type": "token_count",
                        "info": {"total_token_usage": {
                            "input_tokens": input,
                            "cached_input_tokens": 0,
                            "output_tokens": 0,
                            "total_tokens": input
                        }}
                    }
                }),
            ]
            .into_iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join("\n")
                + "\n"
        };
        let active = temp.path().join("active").join("usage.jsonl");
        let archived = temp.path().join("archived").join("usage.jsonl");
        write(&active, &record("session-1", 5));
        write(&archived, &record("session-1", 100));

        let report = report_from_roots(
            ModelUsageRange::Today,
            &[
                (AppKind::Codex, temp.path().join("active")),
                (AppKind::Codex, temp.path().join("archived")),
            ],
            Local::now(),
        );
        assert_eq!(report.groups.len(), 1);
        assert_eq!(report.groups[0].input_tokens, 5);
        assert_eq!(report.groups[0].session_count, 1);
    }

    #[test]
    fn all_range_reports_undated_tokens_separately_from_the_daily_trend() {
        let temp = tempdir().expect("temp");
        write(
            &temp.path().join("codex").join("usage.jsonl"),
            concat!(
                "{\"type\":\"turn_context\",\"payload\":{\"model\":\"gpt-5\"}}\n",
                "{\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":4,\"cached_input_tokens\":1,\"output_tokens\":2,\"total_tokens\":6}}}}\n"
            ),
        );

        let all = report_from_roots(ModelUsageRange::All, &roots(&temp), Local::now());
        assert_eq!(all.groups.len(), 1);
        assert!(all.days.is_empty());
        assert_eq!(
            all.unassigned_tokens,
            ModelUsageTokens {
                input_tokens: 3,
                cache_read_input_tokens: 1,
                cache_creation_input_tokens: 0,
                output_tokens: 2,
                total_tokens: 6,
            }
        );

        let today = report_from_roots(ModelUsageRange::Today, &roots(&temp), Local::now());
        assert!(today.groups.is_empty());
        assert_eq!(today.unassigned_tokens, ModelUsageTokens::default());
    }
}
