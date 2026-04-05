use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use super::cdp::client::CdpClient;

pub async fn set_extra_headers(
    client: &CdpClient,
    session_id: &str,
    headers: &HashMap<String, String>,
) -> Result<(), String> {
    let headers_value: Value = headers
        .iter()
        .map(|(k, v)| (k.clone(), Value::String(v.clone())))
        .collect::<serde_json::Map<String, Value>>()
        .into();

    client
        .send_command(
            "Network.setExtraHTTPHeaders",
            Some(json!({ "headers": headers_value })),
            Some(session_id),
        )
        .await?;

    Ok(())
}

pub async fn set_offline(
    client: &CdpClient,
    session_id: &str,
    offline: bool,
) -> Result<(), String> {
    client
        .send_command(
            "Network.emulateNetworkConditions",
            Some(json!({
                "offline": offline,
                "latency": 0,
                "downloadThroughput": -1,
                "uploadThroughput": -1,
            })),
            Some(session_id),
        )
        .await?;
    Ok(())
}

pub async fn set_content(client: &CdpClient, session_id: &str, html: &str) -> Result<(), String> {
    // Get current frame ID
    let tree_result = client
        .send_command_no_params("Page.getFrameTree", Some(session_id))
        .await?;

    let frame_id = tree_result
        .get("frameTree")
        .and_then(|t| t.get("frame"))
        .and_then(|f| f.get("id"))
        .and_then(|id| id.as_str())
        .ok_or("Could not determine frame ID")?;

    client
        .send_command(
            "Page.setDocumentContent",
            Some(json!({
                "frameId": frame_id,
                "html": html,
            })),
            Some(session_id),
        )
        .await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Domain filter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DomainFilter {
    pub allowed_domains: Vec<String>,
}

impl DomainFilter {
    pub fn new(domains: &str) -> Self {
        let allowed = parse_domain_list(domains);
        Self {
            allowed_domains: allowed,
        }
    }

    pub fn is_allowed(&self, hostname: &str) -> bool {
        if self.allowed_domains.is_empty() {
            return true;
        }
        let hostname = hostname.to_lowercase();
        for pattern in &self.allowed_domains {
            if let Some(suffix) = pattern.strip_prefix("*.") {
                if hostname == suffix || hostname.ends_with(&format!(".{}", suffix)) {
                    return true;
                }
            } else if hostname == *pattern {
                return true;
            }
        }
        false
    }

    pub fn check_url(&self, url: &str) -> Result<(), String> {
        if self.allowed_domains.is_empty() {
            return Ok(());
        }
        let parsed = url::Url::parse(url).map_err(|_| format!("Invalid URL: {}", url))?;
        let hostname = parsed
            .host_str()
            .ok_or_else(|| format!("No hostname in URL: {}", url))?;
        if self.is_allowed(hostname) {
            Ok(())
        } else {
            Err(format!(
                "Domain '{}' is not in the allowed domains list",
                hostname
            ))
        }
    }
}

fn parse_domain_list(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

pub async fn sanitize_existing_pages(
    client: &CdpClient,
    pages: &[super::browser::PageInfo],
    filter: &DomainFilter,
) {
    for page in pages {
        if page.url.is_empty() || page.url == "about:blank" {
            continue;
        }
        if let Ok(parsed) = url::Url::parse(&page.url) {
            if let Some(hostname) = parsed.host_str() {
                if !filter.is_allowed(hostname) {
                    let _ = client
                        .send_command(
                            "Page.navigate",
                            Some(json!({ "url": "about:blank" })),
                            Some(&page.session_id),
                        )
                        .await;
                }
            }
        }
    }
}

pub async fn install_domain_filter_script(
    client: &CdpClient,
    session_id: &str,
    allowed_domains: &[String],
) -> Result<(), String> {
    if allowed_domains.is_empty() {
        return Ok(());
    }

    let domains_json = serde_json::to_string(allowed_domains).unwrap_or("[]".to_string());
    let script = format!(
        r#"(() => {{
            const _allowed = {};
            function _isDomainAllowed(hostname) {{
                hostname = hostname.toLowerCase();
                for (const p of _allowed) {{
                    if (p.startsWith('*.')) {{
                        const suffix = p.slice(2);
                        if (hostname === suffix || hostname.endsWith('.' + suffix)) return true;
                    }} else if (hostname === p) return true;
                }}
                return false;
            }}
            const OrigWS = window.WebSocket;
            window.WebSocket = function(url, protocols) {{
                try {{
                    const u = new URL(url);
                    if (!_isDomainAllowed(u.hostname)) throw new DOMException('WebSocket blocked: ' + u.hostname, 'SecurityError');
                }} catch(e) {{ if (e instanceof DOMException) throw e; }}
                return new OrigWS(url, protocols);
            }};
            window.WebSocket.prototype = OrigWS.prototype;
            const OrigES = window.EventSource;
            if (OrigES) {{
                window.EventSource = function(url, opts) {{
                    try {{
                        const u = new URL(url, location.href);
                        if (!_isDomainAllowed(u.hostname)) throw new DOMException('EventSource blocked: ' + u.hostname, 'SecurityError');
                    }} catch(e) {{ if (e instanceof DOMException) throw e; }}
                    return new OrigES(url, opts);
                }};
                window.EventSource.prototype = OrigES.prototype;
            }}
            const origBeacon = navigator.sendBeacon;
            if (origBeacon) {{
                navigator.sendBeacon = function(url, data) {{
                    try {{
                        const u = new URL(url, location.href);
                        if (!_isDomainAllowed(u.hostname)) return false;
                    }} catch(e) {{ return false; }}
                    return origBeacon.call(navigator, url, data);
                }};
            }}
        }})()"#,
        domains_json,
    );

    client
        .send_command(
            "Page.addScriptToEvaluateOnNewDocument",
            Some(json!({ "source": script })),
            Some(session_id),
        )
        .await?;

    Ok(())
}

/// Enable Fetch-based network interception for domain filtering.
/// This intercepts all requests and checks them against the allowed domains list.
/// The actual handling of `Fetch.requestPaused` events happens in
/// `resolve_fetch_paused` in the actions module.
pub async fn install_domain_filter_fetch(
    client: &CdpClient,
    session_id: &str,
) -> Result<(), String> {
    client
        .send_command(
            "Fetch.enable",
            Some(json!({
                "patterns": [{ "urlPattern": "*" }]
            })),
            Some(session_id),
        )
        .await?;
    Ok(())
}

/// Install both layers of domain filtering on a session:
/// 1. JS patching (WebSocket, EventSource, sendBeacon)
/// 2. Fetch-based network interception
pub async fn install_domain_filter(
    client: &CdpClient,
    session_id: &str,
    allowed_domains: &[String],
) -> Result<(), String> {
    install_domain_filter_script(client, session_id, allowed_domains).await?;
    install_domain_filter_fetch(client, session_id).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Strict SSRF protection
// ---------------------------------------------------------------------------

pub async fn check_ssrf_safe_url(url: &str) -> Result<Vec<String>, String> {
    let parsed = url::Url::parse(url).map_err(|_| format!("Invalid URL: {}", url))?;
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(format!(
            "Strict SSRF protection only allows http/https requests, got '{}'",
            scheme
        ));
    }

    let host = parsed
        .host()
        .ok_or_else(|| format!("No hostname in URL: {}", url))?;

    match host {
        url::Host::Ipv4(ip) => {
            assert_safe_ip(IpAddr::V4(ip), &ip.to_string())?;
            Ok(vec![ip.to_string()])
        }
        url::Host::Ipv6(ip) => {
            assert_safe_ip(IpAddr::V6(ip), &ip.to_string())?;
            Ok(vec![ip.to_string()])
        }
        url::Host::Domain(domain) => {
            let port = parsed.port_or_known_default().unwrap_or(80);
            let resolved = tokio::net::lookup_host((domain, port))
                .await
                .map_err(|e| format!("DNS resolution failed for {}: {}", domain, e))?;

            let mut ips = Vec::new();
            for addr in resolved {
                let ip = addr.ip();
                let ip_str = ip.to_string();
                assert_safe_ip(ip, &ip_str)?;
                if !ips.contains(&ip_str) {
                    ips.push(ip_str);
                }
            }

            if ips.is_empty() {
                return Err(format!(
                    "DNS resolution returned no addresses for {}",
                    domain
                ));
            }

            Ok(ips)
        }
    }
}

fn assert_safe_ip(ip: IpAddr, display: &str) -> Result<(), String> {
    match ip {
        IpAddr::V4(ipv4) => {
            if is_blocked_ipv4(ipv4) {
                return Err(format!(
                    "Blocked private or special-use IPv4 address {}",
                    display
                ));
            }
        }
        IpAddr::V6(ipv6) => {
            if let Some(mapped) = ipv6.to_ipv4_mapped() {
                if is_blocked_ipv4(mapped) {
                    return Err(format!(
                        "Blocked IPv4-mapped private or special-use IPv6 address {}",
                        display
                    ));
                }
                return Ok(());
            }
            if is_blocked_ipv6(ipv6) {
                return Err(format!(
                    "Blocked private or special-use IPv6 address {}",
                    display
                ));
            }
        }
    }

    Ok(())
}

fn is_blocked_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_multicast()
        || octets[0] == 0
}

fn is_blocked_ipv6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
}

// ---------------------------------------------------------------------------
// Console and error tracking
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ConsoleEntry {
    pub level: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct ErrorEntry {
    pub text: String,
    pub url: Option<String>,
    pub line: Option<i64>,
    pub column: Option<i64>,
}

pub struct EventTracker {
    pub console_entries: Vec<ConsoleEntry>,
    pub error_entries: Vec<ErrorEntry>,
    pub max_entries: usize,
}

impl EventTracker {
    pub fn new() -> Self {
        Self {
            console_entries: Vec::new(),
            error_entries: Vec::new(),
            max_entries: 1000,
        }
    }

    pub fn add_console(&mut self, level: &str, text: &str) {
        if self.console_entries.len() >= self.max_entries {
            self.console_entries.remove(0);
        }
        self.console_entries.push(ConsoleEntry {
            level: level.to_string(),
            text: text.to_string(),
        });
    }

    pub fn add_error(
        &mut self,
        text: &str,
        url: Option<&str>,
        line: Option<i64>,
        col: Option<i64>,
    ) {
        if self.error_entries.len() >= self.max_entries {
            self.error_entries.remove(0);
        }
        self.error_entries.push(ErrorEntry {
            text: text.to_string(),
            url: url.map(String::from),
            line,
            column: col,
        });
    }

    pub fn get_console_json(&self) -> Value {
        let entries: Vec<Value> = self
            .console_entries
            .iter()
            .map(|e| json!({ "level": e.level, "text": e.text }))
            .collect();
        json!({ "entries": entries })
    }

    pub fn get_errors_json(&self) -> Value {
        let entries: Vec<Value> = self
            .error_entries
            .iter()
            .map(|e| {
                json!({
                    "text": e.text,
                    "url": e.url,
                    "line": e.line,
                    "column": e.column,
                })
            })
            .collect();
        json!({ "errors": entries })
    }
}

#[cfg(test)]
mod ssrf_tests {
    use super::{check_ssrf_safe_url, is_blocked_ipv4, is_blocked_ipv6};
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn blocks_private_ipv4_ranges() {
        assert!(is_blocked_ipv4(Ipv4Addr::new(127, 0, 0, 1)));
        assert!(is_blocked_ipv4(Ipv4Addr::new(10, 0, 0, 1)));
        assert!(is_blocked_ipv4(Ipv4Addr::new(172, 16, 0, 1)));
        assert!(is_blocked_ipv4(Ipv4Addr::new(192, 168, 1, 1)));
        assert!(is_blocked_ipv4(Ipv4Addr::new(169, 254, 0, 1)));
        assert!(is_blocked_ipv4(Ipv4Addr::new(0, 1, 2, 3)));
    }

    #[test]
    fn allows_public_ipv4() {
        assert!(!is_blocked_ipv4(Ipv4Addr::new(93, 184, 216, 34)));
    }

    #[test]
    fn blocks_private_ipv6_ranges() {
        assert!(is_blocked_ipv6(Ipv6Addr::LOCALHOST));
        assert!(is_blocked_ipv6("fc00::1".parse().unwrap()));
        assert!(is_blocked_ipv6("fe80::1".parse().unwrap()));
    }

    #[test]
    fn allows_public_ipv6() {
        assert!(!is_blocked_ipv6(
            "2606:2800:220:1:248:1893:25c8:1946".parse().unwrap()
        ));
    }

    #[tokio::test]
    async fn rejects_non_http_schemes() {
        let err = check_ssrf_safe_url("file:///etc/passwd").await.unwrap_err();
        assert!(err.contains("only allows http/https"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_filter_exact() {
        let filter = DomainFilter::new("example.com");
        assert!(filter.is_allowed("example.com"));
        assert!(!filter.is_allowed("other.com"));
    }

    #[test]
    fn test_domain_filter_wildcard() {
        let filter = DomainFilter::new("*.example.com");
        assert!(filter.is_allowed("example.com"));
        assert!(filter.is_allowed("api.example.com"));
        assert!(filter.is_allowed("sub.api.example.com"));
        assert!(!filter.is_allowed("other.com"));
    }

    #[test]
    fn test_domain_filter_empty() {
        let filter = DomainFilter::new("");
        assert!(filter.is_allowed("anything.com"));
    }

    #[test]
    fn test_domain_filter_multiple() {
        let filter = DomainFilter::new("example.com, *.api.io");
        assert!(filter.is_allowed("example.com"));
        assert!(filter.is_allowed("api.io"));
        assert!(filter.is_allowed("v1.api.io"));
        assert!(!filter.is_allowed("other.com"));
    }

    #[test]
    fn test_parse_domain_list() {
        let domains = parse_domain_list("A.com, B.com , *.C.com");
        assert_eq!(domains, vec!["a.com", "b.com", "*.c.com"]);
    }

    #[test]
    fn test_event_tracker() {
        let mut tracker = EventTracker::new();
        tracker.add_console("log", "hello");
        tracker.add_error("oops", Some("test.js"), Some(1), Some(5));

        assert_eq!(tracker.console_entries.len(), 1);
        assert_eq!(tracker.error_entries.len(), 1);
    }
}
