use serde::Serialize;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ProbeRunPhase {
    Starting,
    Running,
    Collecting,
    Saving,
    Cancelling,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProbeRunProgressView {
    pub seq: u32,
    pub phase: ProbeRunPhase,
    pub elapsed_ms: u64,
    pub time_limit_ms: u64,
}

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(crate) enum ProbeProgressView {
    Preparing { seq: u32, cancelling: bool },
    Run { #[serde(flatten)] clock: ProbeRunProgressView },
}

/// One monotonic clock is shared by the runner's deadline and live status.
/// It never survives its worker, so restarting cannot revive an old countdown.
pub(super) struct RunProgress {
    seq: u32,
    started: Instant,
    time_limit: Duration,
    phase: Mutex<ProbeRunPhase>,
}

impl RunProgress {
    pub(super) fn new(seq: u32, timeout_seconds: u64) -> Self {
        Self {
            seq,
            started: Instant::now(),
            time_limit: Duration::from_secs(timeout_seconds),
            phase: Mutex::new(ProbeRunPhase::Starting),
        }
    }

    pub(super) fn started(&self) -> Instant { self.started }

    pub(super) fn deadline(&self) -> Instant { self.started + self.time_limit }

    pub(super) fn seq(&self) -> u32 { self.seq }

    pub(super) fn running(&self) { self.advance(ProbeRunPhase::Running); }

    pub(super) fn collecting(&self) { self.advance(ProbeRunPhase::Collecting); }

    pub(super) fn saving(&self) { self.advance(ProbeRunPhase::Saving); }

    fn advance(&self, next: ProbeRunPhase) {
        let mut phase = self.phase.lock().expect("probe progress");
        // A session-id callback can arrive after process exit.
        *phase = (*phase).max(next);
    }

    pub(super) fn view(&self, cancelled: bool) -> ProbeRunProgressView {
        ProbeRunProgressView {
            seq: self.seq,
            phase: if cancelled { ProbeRunPhase::Cancelling }
                else { *self.phase.lock().expect("probe progress") },
            elapsed_ms: self.started.elapsed().as_millis() as u64,
            time_limit_ms: self.time_limit.as_millis() as u64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn views_and_phase_changes_preserve_the_actual_run_deadline() {
        let mut progress = RunProgress::new(2, 180);
        progress.started = Instant::now() - Duration::from_secs(12);
        let deadline = progress.deadline();
        let first = progress.view(false);
        progress.running();
        let second = progress.view(false);
        assert_eq!(first.seq, 2);
        assert_eq!(second.time_limit_ms, 180_000);
        assert!(first.elapsed_ms >= 12_000);
        assert!(second.elapsed_ms >= first.elapsed_ms);
        assert_eq!(progress.deadline(), deadline);
    }

    #[test]
    fn late_events_cannot_revert_collection_or_cancellation() {
        let progress = RunProgress::new(1, 900);
        progress.collecting();
        progress.running();
        assert_eq!(progress.view(false).phase, ProbeRunPhase::Collecting);
        progress.saving();
        progress.running();
        assert_eq!(progress.view(false).phase, ProbeRunPhase::Saving);
        assert_eq!(progress.view(true).phase, ProbeRunPhase::Cancelling);
    }
}
