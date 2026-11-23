use asb_core::contracts::{CodexOperation, CodexProbedOperation};
use reqwest::{blocking::Client, header::HeaderMap, Method, StatusCode, Url};
use std::{collections::BTreeSet, time::Duration};

pub(super) fn discover(
    client: &Client,
    headers: &HeaderMap,
    models_url: &Url,
    base_url: &str,
    is_full_url: bool,
    operations: &BTreeSet<CodexProbedOperation>,
) -> Vec<CodexProbedOperation> {
    std::thread::scope(|scope| {
        let probes: Vec<_> = operations.iter().filter_map(|&candidate| {
            let operation = CodexOperation::from(candidate);
            let target = asb_core::endpoint::codex_operation_endpoint(
                base_url, is_full_url, operation,
            ).ok()?;
            let url = Url::parse(&target).ok()?;
            if url.origin() != models_url.origin() {
                return None;
            }
            Some(scope.spawn(move || {
                advertises_post(client, headers, url, operation == CodexOperation::ImageEdit)
                    .then_some(candidate)
            }))
        }).collect();
        probes.into_iter().filter_map(|probe| probe.join().ok().flatten()).collect()
    })
}

fn advertises_post(client: &Client, headers: &HeaderMap, url: Url, require_json: bool) -> bool {
    let options = |target| client.request(Method::OPTIONS, target)
        .headers(headers.clone()).timeout(Duration::from_secs(2)).send();
    let Ok(response) = options(url.clone()) else { return false };
    if !(response.status().is_success() || response.status() == StatusCode::METHOD_NOT_ALLOWED)
        || !header_contains(response.headers(), "allow", "POST")
    {
        return false;
    }
    // The gateway forwards JSON, so a multipart-only image edit is insufficient.
    if (require_json || response.headers().contains_key("accept-post"))
        && !header_contains(response.headers(), "accept-post", "application/json")
    {
        return false;
    }
    let mut missing = url;
    let Ok(mut path) = missing.path_segments_mut() else { return false };
    path.pop().push("asb-capability-control-404");
    drop(path);
    // A general CORS or wildcard handler can return Allow on every path.
    options(missing).is_ok_and(|response| response.status() == StatusCode::NOT_FOUND
        && !header_contains(response.headers(), "allow", "POST"))
}

fn header_contains(headers: &HeaderMap, name: &'static str, value: &str) -> bool {
    headers.get_all(name).iter().filter_map(|header| header.to_str().ok())
        .flat_map(|header| header.split(','))
        .any(|item| item.split(';').next().unwrap_or_default().trim().eq_ignore_ascii_case(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_route_specific_options_without_sending_post() {
        use tiny_http::{Header, Response, Server};

        let server = Server::http("127.0.0.1:0").unwrap();
        let url = Url::parse(&format!("http://{}/v1/images/edits", server.server_addr())).unwrap();
        let upstream = std::thread::spawn(move || {
            for (path, status) in [("/v1/images/edits", 204), ("/v1/images/asb-capability-control-404", 404)] {
                let request = server.recv_timeout(Duration::from_secs(3)).unwrap().unwrap();
                assert_eq!(request.method(), &tiny_http::Method::Options);
                assert_eq!(request.url(), path);
                let mut response = Response::empty(tiny_http::StatusCode(status));
                if status == 204 {
                    response.add_header(Header::from_bytes("Allow", "POST, OPTIONS").unwrap());
                    response.add_header(Header::from_bytes("Accept-Post", "application/json").unwrap());
                }
                request.respond(response).unwrap();
            }
        });
        let client = Client::builder().no_proxy().build().unwrap();
        assert!(advertises_post(&client, &HeaderMap::new(), url, true));
        upstream.join().unwrap();
    }
}
