use super::contracts::*;

#[derive(Default)]
pub(super) struct Accumulator {
    metrics: RequestUsageMetrics,
    speeds: Vec<f64>,
    first_outputs: Vec<f64>,
    durations: Vec<f64>,
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
        a.requests += b.requests;
        a.completed += b.completed;
        a.failed += b.failed;
        a.disconnected += b.disconnected;
        a.incomplete += b.incomplete;
        a.cache_samples += b.cache_samples;
        a.retry_requests += b.retry_requests;
        a.retries += b.retries;
        a.attempt_samples += b.attempt_samples;
        a.recovered_requests += b.recovered_requests;
        a.rate_limited += b.rate_limited;
        a.upstream_errors += b.upstream_errors;
        a.unclassified_failures += b.unclassified_failures;
        a.cost_samples += b.cost_samples;
        a.pricing_errors += b.pricing_errors;
        self.speeds.append(&mut other.speeds);
        self.first_outputs.append(&mut other.first_outputs);
        self.durations.append(&mut other.durations);
        self.cache_input = self.cache_input.saturating_add(other.cache_input);
        self.cache_read = self.cache_read.saturating_add(other.cache_read);
        self.cost_micros = self.cost_micros.saturating_add(other.cost_micros);
        for i in 0..5 {
            self.token_sums[i] = self.token_sums[i].saturating_add(other.token_sums[i]);
            self.token_samples[i] += other.token_samples[i];
        }
    }

    pub(super) fn add(&mut self, row: &RequestUsageRow) {
        self.metrics.requests += 1;
        match row.outcome {
            RequestOutcome::Completed => self.add_completed(row),
            RequestOutcome::Disconnected => self.metrics.disconnected += 1,
            RequestOutcome::Failed => {
                self.metrics.failed += 1;
                match row.status {
                    Some(206) => self.metrics.incomplete += 1,
                    Some(429) => self.metrics.rate_limited += 1,
                    Some(500..=599) => self.metrics.upstream_errors += 1,
                    None => self.metrics.unclassified_failures += 1,
                    _ => {},
                }
            }
        }
        let retries = row.attempts.len().saturating_sub(1) as u64;
        self.metrics.attempt_samples += u64::from(!row.attempts.is_empty());
        self.metrics.retries += retries;
        if retries > 0 {
            self.metrics.retry_requests += 1;
            self.metrics.recovered_requests += u64::from(row.outcome == RequestOutcome::Completed);
        }
        for (index, value) in [row.input_tokens, row.cache_read_tokens,
            row.cache_creation_tokens, row.output_tokens, row.reasoning_tokens].into_iter().enumerate()
        {
            if let Some(value) = value {
                self.token_sums[index] = self.token_sums[index].saturating_add(value);
                self.token_samples[index] += 1;
            }
        }
        if let Some(micros) = row.estimated_cost_usd.as_deref()
            .and_then(|cost| asb_core::contracts::decimal_micros(cost).ok())
        {
            self.cost_micros = self.cost_micros.saturating_add(micros);
            self.metrics.cost_samples += 1;
        }
        self.metrics.pricing_errors += u64::from(row.pricing_error.is_some());
    }

    fn add_completed(&mut self, row: &RequestUsageRow) {
        self.metrics.completed += 1;
        if let Some(speed) = row.output_tokens_per_second { self.speeds.push(speed); }
        if let Some(first) = row.first_output_ms { self.first_outputs.push(first as f64); }
        if row.duration_ms > 0 { self.durations.push(row.duration_ms as f64); }
        if let Some((input, read)) = row.input_tokens.zip(row.cache_read_tokens)
            .filter(|(input, read)| *input > 0 && read <= input)
        {
            self.cache_input = self.cache_input.saturating_add(input);
            self.cache_read = self.cache_read.saturating_add(read);
            self.metrics.cache_samples += 1;
        }
    }

    pub(super) fn finish(mut self) -> RequestUsageMetrics {
        self.speeds.sort_by(f64::total_cmp);
        self.first_outputs.sort_by(f64::total_cmp);
        self.durations.sort_by(f64::total_cmp);
        let m = &mut self.metrics;
        m.completion_rate = percentage(m.completed, m.completed + m.failed);
        m.output_rate_p50 = percentile(&self.speeds, 0.5);
        m.output_rate_p10 = percentile(&self.speeds, 0.1);
        m.speed_samples = self.speeds.len();
        m.first_output_p50_ms = percentile(&self.first_outputs, 0.5);
        m.first_output_p95_ms = percentile(&self.first_outputs, 0.95);
        m.first_output_samples = self.first_outputs.len();
        m.duration_p50_ms = percentile(&self.durations, 0.5);
        m.duration_p95_ms = percentile(&self.durations, 0.95);
        m.duration_samples = self.durations.len();
        m.cache_hit_rate = percentage(self.cache_read, self.cache_input);
        m.estimated_cost_usd = (m.cost_samples > 0).then(|| format_micros(self.cost_micros));
        m.tokens = ["input", "cacheRead", "cacheCreation", "output", "reasoning"]
            .into_iter().enumerate().map(|(index, kind)| RequestUsageTokenTotal {
                kind, tokens: (self.token_samples[index] > 0).then_some(self.token_sums[index]),
                samples: self.token_samples[index],
            }).collect();
        self.metrics
    }
}

fn format_micros(micros: u64) -> String {
    format!("{}.{:06}", micros / 1_000_000, micros % 1_000_000)
}

pub(super) fn percentage(numerator: u64, denominator: u64) -> Option<f64> {
    (denominator > 0).then(|| numerator as f64 * 100.0 / denominator as f64)
}

/// Linear interpolation over individual requests, never averages of group percentiles.
fn percentile(sorted: &[f64], fraction: f64) -> Option<f64> {
    if sorted.is_empty() { return None; }
    let position = (sorted.len() - 1) as f64 * fraction;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    Some(sorted[lower] + (sorted[upper] - sorted[lower]) * (position - lower as f64))
}
