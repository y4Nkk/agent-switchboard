//! Shared HTTP admission and decoding; provider business lives in client modules.
use super::*;

pub(super) fn classify(
    request: &Request,
) -> Result<
    (UpstreamProtocol, AppKind, Option<CodexOperation>),
    (Option<UpstreamProtocol>, u16, &'static str),
> {
    let operation = codex_request(request.url()).map(|path| path.operation);
    let protocol = client_protocol(request.url());
    let method = if operation.is_some_and(CodexOperation::is_model_list) { Method::Get } else { Method::Post };
    if request.method() != &method {
        return Err((protocol, 405, "此本机协议网关操作不接受该 HTTP 方法"));
    }
    let protocol = protocol.ok_or((None, 404, "本机协议网关不处理该路径"))?;
    let app = span_app(protocol).ok_or((
        Some(protocol),
        404,
        "本机协议网关不接收 Chat Completions 客户端请求",
    ))?;
    Ok((protocol, app, operation))
}

pub(super) fn authorized_route(
    request: &Request,
    inner: &GatewayInner,
    protocol: UpstreamProtocol,
    app: AppKind,
) -> Option<(ActiveRoute, Vec<ActiveRoute>)> {
    let token = request_capability(request, protocol)?;
    let routes = inner.routes.read().ok()?;
    let route = routes
        .get(&app)
        .filter(|route| constant_time_equal(route.client_token.as_bytes(), token.as_bytes()))?
        .clone();
    let candidates = inner.candidates_for(app, &route);
    Some((route, candidates))
}

pub(super) fn read_body(request: &mut Request) -> Result<Vec<u8>, (u16, String)> {
    let raw = request.take_body();
    content_encoding::decode_request_body(request.headers(), &raw, MAX_REQUEST_BYTES as usize)
        .map_err(|error| match error {
            content_encoding::DecodeError::TooLarge => {
                (413, "解压后的请求体超过本机协议网关限制".into())
            }
            content_encoding::DecodeError::Unsupported(coding) => {
                (415, format!("不支持请求 Content-Encoding: {coding}"))
            }
            content_encoding::DecodeError::Invalid(error) => {
                (422, format!("无法解压请求体: {error}"))
            }
        })
}
