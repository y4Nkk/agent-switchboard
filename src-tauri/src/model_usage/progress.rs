use asb_core::contracts::{ModelUsagePhase, ModelUsageProgress};
use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

pub(crate) fn initial() -> ModelUsageProgress {
    ModelUsageProgress { phase: ModelUsagePhase::Discovering, completed_sources: 0, total_sources: None,
        completed_bytes: 0, total_bytes: None, completed_calls: 0, total_calls: None }
}

pub(crate) struct Progress<'a> {
    value: RefCell<ModelUsageProgress>,
    sent: Cell<Instant>,
    publish: &'a dyn Fn(ModelUsageProgress),
}

impl<'a> Progress<'a> {
    pub(crate) fn new(publish: &'a dyn Fn(ModelUsageProgress)) -> Self {
        Self { value: RefCell::new(initial()), sent: Cell::new(Instant::now()), publish }
    }

    pub(crate) fn phase(&self, phase: ModelUsagePhase) {
        let mut value = self.value.borrow_mut();
        value.phase = phase;
        value.completed_bytes = 0;
        value.total_bytes = None;
        value.completed_calls = 0;
        value.total_calls = None;
        drop(value);
        self.send(true);
    }

    pub(crate) fn sources(&self, total: usize) {
        self.value.borrow_mut().total_sources = Some(total as u64);
        self.phase(ModelUsagePhase::Checking);
    }

    pub(crate) fn source_done(&self) {
        self.value.borrow_mut().completed_sources += 1;
        self.send(true);
    }

    pub(crate) fn bytes(&self, phase: ModelUsagePhase, total: u64) {
        self.phase(phase);
        self.value.borrow_mut().total_bytes = Some(total);
        self.send(true);
    }

    pub(crate) fn advance_bytes(&self, bytes: u64) {
        self.value.borrow_mut().completed_bytes += bytes;
        self.send(false);
    }

    pub(crate) fn calls(&self, total: usize) {
        self.phase(ModelUsagePhase::Aggregating);
        self.value.borrow_mut().total_calls = Some(total as u64);
        self.send(true);
    }

    pub(crate) fn call_done(&self) {
        self.value.borrow_mut().completed_calls += 1;
        self.send(false);
    }

    fn send(&self, force: bool) {
        if force || self.sent.get().elapsed() >= Duration::from_millis(100) {
            (self.publish)(self.value.borrow().clone());
            self.sent.set(Instant::now());
        }
    }
}
