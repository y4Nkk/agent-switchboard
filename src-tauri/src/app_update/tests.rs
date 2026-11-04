use super::*;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

struct Payload(Arc<AtomicBool>);
impl Drop for Payload { fn drop(&mut self) { self.0.store(true, Ordering::SeqCst); } }

#[tokio::test]
async fn cancelling_a_stalled_download_drops_its_partial_payload() {
    let dropped = Arc::new(AtomicBool::new(false));
    let started = Arc::new(AtomicBool::new(false));
    let (cancel, signal) = watch::channel(false);
    let payload = Payload(dropped.clone());
    let polling = started.clone();
    let pending = tokio::spawn(until_cancelled(signal, async move {
        let _payload = payload;
        polling.store(true, Ordering::SeqCst);
        std::future::pending::<()>().await;
        Ok(Vec::<u8>::new())
    }));
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while !started.load(Ordering::SeqCst) { tokio::task::yield_now().await; }
        cancel.send(true).unwrap();
        assert!(pending.await.unwrap().unwrap().is_none());
    }).await.unwrap();
    assert!(dropped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn cancellation_before_response_headers_never_polls_the_download() {
    let polled = Arc::new(AtomicBool::new(false));
    let flag = polled.clone();
    let (cancel, signal) = watch::channel(false);
    cancel.send(true).unwrap();
    let result = until_cancelled(signal, async move {
        flag.store(true, Ordering::SeqCst);
        Ok(vec![1u8])
    }).await.unwrap();
    assert!(result.is_none());
    assert!(!polled.load(Ordering::SeqCst));
}

#[tokio::test]
async fn completed_payloads_and_real_errors_pass_through_without_becoming_cancellation() {
    let (_cancel, signal) = watch::channel(false);
    assert_eq!(until_cancelled(signal.clone(), async { Ok(vec![1u8, 2]) }).await.unwrap(), Some(vec![1, 2]));
    let error = until_cancelled::<Vec<u8>>(signal, async {
        Err(CommandError::new("fixture-failed", "network failed"))
    }).await.unwrap_err();
    assert_eq!(error.code, "fixture-failed");
}

#[test]
fn signature_failures_keep_their_distinct_error_code() {
    let error = failure("download", tauri_plugin_updater::Error::SignatureUtf8("invalid fixture".into()));
    assert_eq!(error.code, "updater-signature-invalid");
    assert_eq!(error.message_key.as_deref(), Some("errors.updater.signatureInvalid"));
    assert!(error.message.contains("invalid fixture"));
}
