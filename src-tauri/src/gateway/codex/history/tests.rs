use super::*;
use serde_json::json;

fn binding(session: &str) -> HistoryBinding {
    HistoryBinding::for_request(
        json!({"client_metadata":{"session_id":session}}).to_string().as_bytes(),
        None,
    )
}

fn record(history: &CodexToolHistory, session: &str, response: &str, name: &str) {
    history.record_response(&binding(session), json!({
        "id":response,"status":"completed","output":[{
            "type":"function_call","call_id":"call-1","name":name,
            "arguments":json!({"owner":session}).to_string(),"status":"completed",
        }],
    }).to_string().as_bytes());
}

fn continuation() -> Value {
    json!({"input":[{"type":"function_call_output","call_id":"call-1","output":"result"}]})
}

#[test]
fn call_id_fallback_never_restores_another_sessions_arguments() {
    let history = CodexToolHistory::default();
    record(&history, "first", "response-first", "first-tool");
    record(&history, "second", "response-second", "second-tool");
    let mut body = continuation();
    assert_eq!(history.enrich(&binding("second"), &mut body).unwrap(), 1);
    assert_eq!(body["input"][0]["name"], "second-tool");
    assert_eq!(body["input"][0]["arguments"], json!({"owner":"second"}).to_string());
}

#[test]
fn identical_response_ids_are_independent_between_sessions() {
    let history = CodexToolHistory::default();
    record(&history, "first", "same-response", "first-tool");
    record(&history, "second", "same-response", "second-tool");
    for (session, tool) in [("first", "first-tool"), ("second", "second-tool")] {
        let mut body = continuation();
        body["previous_response_id"] = json!("same-response");
        assert_eq!(history.enrich(&binding(session), &mut body).unwrap(), 1);
        assert_eq!(body["input"][0]["name"], tool);
        assert!(body.get("previous_response_id").is_none());
    }
}

#[test]
fn ambiguous_calls_require_an_explicit_response_in_the_same_session() {
    let history = CodexToolHistory::default();
    record(&history, "session", "one", "one-tool");
    record(&history, "session", "two", "two-tool");
    let mut body = continuation();
    assert_eq!(history.enrich(&binding("session"), &mut body).unwrap(), 0);
    body["previous_response_id"] = json!("two");
    assert_eq!(history.enrich(&binding("session"), &mut body).unwrap(), 1);
    assert_eq!(body["input"][0]["name"], "two-tool");
    let mut other = continuation();
    other["previous_response_id"] = json!("two");
    assert!(history.enrich(&binding("other"), &mut other).is_err());
}

#[test]
fn anonymous_requests_never_share_tool_history() {
    let history = CodexToolHistory::default();
    let anonymous = HistoryBinding::default();
    history.record_response(&anonymous, json!({
        "id":"anonymous","status":"completed","output":[{
            "type":"function_call","call_id":"call-1","name":"secret","arguments":"{}",
        }],
    }).to_string().as_bytes());
    assert!(history.inner.read().unwrap().entries.is_empty());
    let mut body = continuation();
    assert_eq!(history.enrich(&anonymous, &mut body).unwrap(), 0);
    body["previous_response_id"] = json!("anonymous");
    assert!(history.enrich(&anonymous, &mut body).is_err());
}

#[test]
fn incomplete_tool_calls_never_enter_continuation_history() {
    let history = CodexToolHistory::default();
    let binding = binding("session");
    let mut response = json!({
        "id":"response-1","status":"incomplete","output":[{
            "type":"function_call","call_id":"call-1","name":"read_file",
            "arguments":"{","status":"incomplete",
        }],
    });
    history.record_response(&binding, response.to_string().as_bytes());
    assert!(history.inner.read().unwrap().entries.is_empty());
    response["status"] = json!("completed");
    response["output"][0]["arguments"] = json!("{}");
    response["output"][0]["status"] = json!("completed");
    history.record_response(&binding, response.to_string().as_bytes());
    assert!(history.inner.read().unwrap().entry("response-1", &binding).is_some());
}
