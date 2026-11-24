use serde::{Deserialize, Serialize};

use asb_core::extensions::contracts::{McpDefinition, McpRegistrySource, SecretValue};
use asb_core::extensions::validate::{mcp_registry_endpoint, validate_mcp_registry_source};

use super::sources::{fetch_source_bytes, HttpFetch, SourceError, MAX_DIRECTORY_DOWNLOAD};

const SERVERS: &str = "https://registry.modelcontextprotocol.io/v0.1/servers";
const PAGE_SIZE: &str = "20";

#[derive(Deserialize)]
struct RegistryPage {
    servers: Vec<RegistryResponse>,
    metadata: Pagination,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pagination {
    next_cursor: Option<String>,
}

#[derive(Deserialize)]
struct RegistryResponse {
    server: RegistryRelease,
    #[serde(rename = "_meta")]
    meta: Option<ResponseMeta>,
}

#[derive(Deserialize)]
struct ResponseMeta {
    #[serde(rename = "io.modelcontextprotocol.registry/official")]
    official: Option<OfficialMeta>,
}

#[derive(Deserialize)]
struct OfficialMeta {
    status: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RegistryRelease {
    name: String,
    pub version: String,
    title: Option<String>,
    description: String,
    website_url: Option<String>,
    repository: Option<Repository>,
    #[serde(default)]
    remotes: Vec<Remote>,
}

#[derive(Deserialize)]
struct Repository {
    url: String,
}

#[derive(Deserialize)]
struct Remote {
    #[serde(rename = "type")]
    transport: String,
    url: String,
    #[serde(default)]
    headers: Vec<Header>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Header {
    name: String,
    #[serde(default)]
    is_required: bool,
}

impl Remote {
    fn endpoint(&self) -> Option<reqwest::Url> {
        if !matches!(self.transport.as_str(), "streamable-http" | "sse")
            || self.required_headers().any(|name| reqwest::header::HeaderName::from_bytes(name.as_bytes()).is_err())
        {
            return None;
        }
        mcp_registry_endpoint(&self.url)
    }

    fn required_headers(&self) -> impl Iterator<Item = &str> {
        self.headers.iter().filter(|header| header.is_required).map(|header| header.name.as_str())
    }
}

#[derive(Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpRegistryConnection {
    transport: &'static str,
    endpoint: String,
    required_headers: Vec<String>,
}

impl RegistryRelease {
    fn source(&self, endpoint: String) -> Option<McpRegistrySource> {
        let source = McpRegistrySource { name: self.name.clone(), version: self.version.clone(), endpoint };
        validate_mcp_registry_source(&source).ok().map(|()| source)
    }

    pub fn default_source(&self) -> Option<McpRegistrySource> {
        let mut endpoints: Vec<_> = self.remotes.iter()
            .filter(|remote| remote.transport == "streamable-http" && remote.required_headers().next().is_none())
            .filter_map(|remote| remote.endpoint().map(|url| url.to_string()))
            .collect();
        endpoints.sort_unstable();
        endpoints.dedup();
        let [endpoint] = endpoints.as_slice() else { return None };
        self.source(endpoint.clone())
    }

    pub fn configured_source(&self, definition: &McpDefinition) -> Option<McpRegistrySource> {
        let (transport, url, headers, bearer) = match definition {
            McpDefinition::Http { url, headers, bearer, .. } => ("streamable-http", url, headers, bearer.as_ref()),
            McpDefinition::ClaudeSse { url, headers } => ("sse", url, headers, None),
            _ => return None,
        };
        let endpoint = mcp_registry_endpoint(url)?;
        let matches = self.remotes.iter().any(|remote| {
            remote.transport == transport && remote.endpoint().as_ref() == Some(&endpoint)
                && remote.required_headers().all(|required| {
                    headers.iter().any(|(name, value)| name.eq_ignore_ascii_case(required) && configured(value))
                        || (required.eq_ignore_ascii_case("authorization") && bearer.is_some_and(configured))
                })
        });
        if !matches { return None; }
        self.source(endpoint.to_string())
    }

    pub fn connections(&self) -> Vec<McpRegistryConnection> {
        let mut connections: Vec<_> = self.remotes.iter().filter_map(|remote| {
            let endpoint = remote.endpoint()?.to_string();
            let transport = if remote.transport == "sse" { "claudeSse" } else { "http" };
            let mut required_headers: Vec<_> = remote.required_headers().map(str::to_ascii_lowercase).collect();
            required_headers.sort_unstable();
            required_headers.dedup();
            Some(McpRegistryConnection { transport, endpoint, required_headers })
        }).collect();
        connections.sort_unstable();
        connections.dedup();
        connections
    }

    pub fn docs(&self) -> Option<String> {
        self.website_url.as_deref().and_then(docs_url)
            .or_else(|| self.repository.as_ref().and_then(|repo| docs_url(&repo.url)))
    }
}

fn configured(value: &SecretValue) -> bool {
    !matches!(value, SecretValue::Plain { value } if value.trim().is_empty())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpRegistryEntry {
    pub name: String,
    pub version: String,
    pub title: Option<String>,
    pub description: String,
    pub docs: Option<String>,
    pub source: Option<McpRegistrySource>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpRegistryPage {
    items: Vec<McpRegistryEntry>,
    next_cursor: Option<String>,
}

fn docs_url(value: &str) -> Option<String> {
    let url = reqwest::Url::parse(value).ok()?;
    (value.len() <= 2048 && !value.chars().any(char::is_control)
        && url.scheme() == "https" && url.host_str().is_some()
        && url.username().is_empty() && url.password().is_none()).then(|| url.to_string())
}

fn registry_entry(response: RegistryResponse) -> Option<McpRegistryEntry> {
    let server = active_release(response)?;
    let source = server.default_source();
    let docs = server.docs();
    Some(McpRegistryEntry {
        name: server.name, version: server.version,
        title: server.title.map(|title| text(&title, 128)).filter(|title| !title.is_empty()),
        description: text(&server.description, 2000),
        docs,
        source,
    })
}

fn active_release(response: RegistryResponse) -> Option<RegistryRelease> {
    (response.meta?.official?.status.as_deref() == Some("active")).then_some(response.server)
}

fn text(value: &str, limit: usize) -> String {
    value.trim().chars().filter(|ch| *ch != '\0').take(limit).collect()
}

fn read<T: serde::de::DeserializeOwned>(url: &reqwest::Url, fetch: HttpFetch) -> Result<T, SourceError> {
    let bytes = fetch_source_bytes(url.as_str(), MAX_DIRECTORY_DOWNLOAD, fetch)?;
    serde_json::from_slice(&bytes).map_err(|error| SourceError::Unreachable(format!("MCP Registry: {error}")))
}

pub fn search(query: &str, cursor: Option<&str>, fetch: HttpFetch) -> Result<McpRegistryPage, SourceError> {
    let query = query.trim();
    if query.chars().count() > 200 || query.chars().any(char::is_control)
        || cursor.is_some_and(|value| value.len() > 2048 || value.chars().any(char::is_control)) {
        return Err(SourceError::Rejected("MCP Registry 搜索参数无效".into()));
    }
    let mut url = reqwest::Url::parse(SERVERS).expect("static Registry URL");
    {
        let mut params = url.query_pairs_mut();
        params.append_pair("version", "latest").append_pair("limit", PAGE_SIZE);
        if !query.is_empty() { params.append_pair("search", query); }
        if let Some(cursor) = cursor { params.append_pair("cursor", cursor); }
    }
    let page: RegistryPage = read(&url, fetch)?;
    Ok(McpRegistryPage {
        items: page.servers.into_iter().filter_map(registry_entry).collect(),
        next_cursor: page.metadata.next_cursor.filter(|next| !next.is_empty() && Some(next.as_str()) != cursor),
    })
}

pub(crate) fn latest(source: &McpRegistrySource, fetch: HttpFetch) -> Result<RegistryRelease, SourceError> {
    let mut url = reqwest::Url::parse(SERVERS).expect("static Registry URL");
    url.path_segments_mut().expect("static hierarchical Registry URL")
        .extend([source.name.as_str(), "versions", "latest"]);
    let response: RegistryResponse = read(&url, fetch)?;
    if response.server.name != source.name {
        return Err(SourceError::Rejected("MCP Registry 返回了不同名称的条目".into()));
    }
    active_release(response).ok_or_else(|| SourceError::Rejected("MCP Registry 最新版本已弃用或下架".into()))
}
