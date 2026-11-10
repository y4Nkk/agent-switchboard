use super::contracts::*;
use std::collections::BTreeSet;

#[derive(Default)]
pub(super) struct Accumulator {
    metrics: RequestUsageMetrics,
    speeds: Vec<f64>,
    first_tokens: Vec<f64>,
    first_contents: Vec<f64>,
    durations: Vec<f64>,
    outcome_operations: BTreeSet<String>,
    speed_operations: BTreeSet<String>,
    first_token_operations: BTreeSet<String>,
    duration_operations: BTreeSet<String>,
    cache_read: u64,
    cache_input: u64,
    cost_micros: u64,
    token_sums: [u64; 5],
    token_samples: [u64; 5],
}

impl Accumulator {
    pub(super) fn merge(&mut self, mut other: Self) {
        let a = &mut self.metrics;
        let b = other.metrics;
        a.records += b.records;
        a.succeeded += b.succeeded;
        a.failed += b.failed;
        a.unknown += b.unknown;
        a.cache_samples += b.cache_samples;
        a.retry_records += b.retry_records;
        a.retries += b.retries;
        a.attempt_samples += b.attempt_samples;
        a.recovered_records += b.recovered_records;
        a.rate_limited += b.rate_limited;
        a.upstream_errors += b.upstream_errors;
        a.unclassified_failures += b.unclassified_failures;
        a.cost_samples += b.cost_samples;
        self.speeds.append(&mut other.speeds);
        self.first_tokens.append(&mut other.first_tokens);
        self.first_contents.append(&mut other.first_contents);
        self.durations.append(&mut other.durations);
        self.outcome_operations.append(&mut other.outcome_operations);
        self.speed_operations.append(&mut other.speed_operations);
        self.first_token_operations.append(&mut other.first_token_operations);
        self.duration_operations.append(&mut other.duration_operations);
        self.cache_input = self.cache_input.saturating_add(other.cache_input);
        self.cache_read = self.cache_read.saturating_add(other.cache_read);
        self.cost_micros = self.cost_micros.saturating_add(other.cost_micros);
        for i in 0..5 {
            self.token_sums[i] = self.token_sums[i].saturating_add(other.token_sums[i]);
            self.token_samples[i] += other.token_samples[i];
        }
    }

    pub(super) fn add(&mut self, row: &RequestUsageRow) {
        self.metrics.records += 1;
        if row.outcome != RequestOutcome::Unknown { self.outcome_operations.insert(row.event_name.clone()); }
        match row.outcome {
            RequestOutcome::Succeeded => self.add_succeeded(row),
            RequestOutcome::Unknown => self.metrics.unknown += 1,
            RequestOutcome::Failed => {
                self.metrics.failed += 1;
                match row.status {
                    Some(429) => self.metrics.rate_limited += 1,
                    Some(500..=599) => self.metrics.upstream_errors += 1,
                    None => self.metrics.unclassified_failures += 1,
                    _ => {}
                }
            }
        }
        if let Some(attempts) = row.attempt_count {
            self.metrics.attempt_samples += 1;
            if attempts > 1 {
                self.metrics.retry_records += 1;
                self.metrics.retries += attempts - 1;
                self.metrics.recovered_records += u64::from(row.outcome == RequestOutcome::Succeeded);
            }
        }
        for (index, value) in [row.input_tokens, row.cache_read_tokens, row.cache_creation_tokens,
            row.output_tokens, row.reasoning_tokens].into_iter().enumerate() {
            if let Some(value) = value {
                self.token_sums[index] = self.token_sums[index].saturating_add(value);
                self.token_samples[index] += 1;
            }
        }
        if let Some(micros) = row.estimated_cost_usd.as_deref()
            .and_then(|cost| asb_core::contracts::decimal_micros(cost).ok()) {
            self.cost_micros = self.cost_micros.saturating_add(micros);
            self.metrics.cost_samples += 1;
        }
    }

    fn add_succeeded(&mut self, row: &RequestUsageRow) {
        self.metrics.succeeded += 1;
        if let Some(speed) = row.output_tokens_per_second {
            self.speed_operations.insert(row.event_name.clone()); self.speeds.push(speed);
        }
        if let Some(first) = row.first_token_ms {
            self.first_token_operations.insert(row.event_name.clone()); self.first_tokens.push(first);
        }
        if let Some(first) = row.first_content_ms { self.first_contents.push(first); }
        if let Some(duration) = row.duration_ms.filter(|value| *value > 0.0) {
            self.duration_operations.insert(row.event_name.clone()); self.durations.push(duration);
        }
        if let Some((input, read)) = row.input_tokens.zip(row.cache_read_tokens)
            .filter(|(input, read)| *input > 0 && read <= input) {
            self.cache_input = self.cache_input.saturating_add(input);
            self.cache_read = self.cache_read.saturating_add(read);
            self.metrics.cache_samples += 1;
        }
    }

    pub(super) fn finish(mut self) -> RequestUsageMetrics {
        self.speeds.sort_by(f64::total_cmp);
        self.first_tokens.sort_by(f64::total_cmp);
        self.first_contents.sort_by(f64::total_cmp);
        self.durations.sort_by(f64::total_cmp);
        let m = &mut self.metrics;
        m.success_ratio = percentage(m.succeeded, m.succeeded + m.failed);
        m.output_rate_p50 = percentile(&self.speeds, 0.5);
        m.output_rate_p10 = percentile(&self.speeds, 0.1);
        m.speed_samples = self.speeds.len();
        m.first_token_p50_ms = percentile(&self.first_tokens, 0.5);
        m.first_token_p95_ms = percentile(&self.first_tokens, 0.95);
        m.first_token_samples = self.first_tokens.len();
        m.first_content_p50_ms = percentile(&self.first_contents, 0.5);
        m.first_content_samples = self.first_contents.len();
        m.duration_p50_ms = percentile(&self.durations, 0.5);
        m.duration_p95_ms = percentile(&self.durations, 0.95);
        m.duration_samples = self.durations.len();
        m.cache_hit_rate = percentage(self.cache_read, self.cache_input);
        m.estimated_cost_usd = (m.cost_samples > 0).then(|| format!("{}.{:06}",
            self.cost_micros / 1_000_000, self.cost_micros % 1_000_000));
        m.tokens = ["input", "cacheRead", "cacheCreation", "output", "reasoning"]
            .into_iter().enumerate().map(|(index, kind)| RequestUsageTokenTotal {
                kind, tokens: (self.token_samples[index] > 0).then_some(self.token_sums[index]),
                samples: self.token_samples[index],
            }).collect();
        // Native operations can have different retry/transport boundaries even within one UI scope.
        m.mixed_measurements = [&self.outcome_operations, &self.speed_operations,
            &self.first_token_operations, &self.duration_operations].iter().any(|sources| sources.len() > 1);
        if self.outcome_operations.len() > 1 { m.success_ratio = None; }
        if self.speed_operations.len() > 1 { m.output_rate_p50 = None; m.output_rate_p10 = None; }
        if self.first_token_operations.len() > 1 { m.first_token_p50_ms = None; m.first_token_p95_ms = None; }
        if self.duration_operations.len() > 1 { m.duration_p50_ms = None; m.duration_p95_ms = None; }
        self.metrics
    }
}

pub(super) fn percentage(numerator: u64, denominator: u64) -> Option<f64> {
    (denominator > 0).then(|| numerator as f64 * 100.0 / denominator as f64)
}

fn percentile(sorted: &[f64], fraction: f64) -> Option<f64> {
    if sorted.is_empty() { return None; }
    let position = (sorted.len() - 1) as f64 * fraction;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    Some(sorted[lower] + (sorted[upper] - sorted[lower]) * (position - lower as f64))
}
