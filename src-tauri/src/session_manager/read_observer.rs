use std::sync::{Mutex, MutexGuard, TryLockError};
use std::time::Duration;

/// Waiting for a shared reader must remain cancellable before any I/O starts.
pub(super) fn lock<'a, T>(gate: &'a Mutex<T>, checkpoint: &dyn Fn() -> Result<(), String>) -> Result<MutexGuard<'a, T>, String> {
    loop {
        checkpoint()?;
        match gate.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(TryLockError::Poisoned(_)) => return Err("会话读取锁不可用".into()),
            Err(TryLockError::WouldBlock) => std::thread::sleep(Duration::from_millis(10)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_interrupts_waiting_for_an_owned_lock() {
        let gate = Mutex::new(());
        let _held = gate.lock().unwrap();
        let attempts = std::cell::Cell::new(0);
        let result = lock(&gate, &|| {
            attempts.set(attempts.get() + 1);
            if attempts.get() == 2 { Err("cancelled".into()) } else { Ok(()) }
        });
        assert!(result.is_err());
        assert_eq!(attempts.get(), 2);
    }
}
