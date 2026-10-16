//! Proxy URL construction and per-URL proxy routing rules.

use url::Url;

use super::app_settings::{AppSettings, ProxyListMode, ProxyProtocol};

/// Pure helper that turns the proxy settings into a yt-dlp `--proxy` value and
/// decides whether a given URL should be routed through the proxy.
pub struct ProxyRules;

impl ProxyRules {
    /// Builds the `scheme://host:port` value for yt-dlp, or `None` when the proxy
    /// is disabled or the host/port are invalid.
    pub fn build_proxy_url(settings: &AppSettings) -> Option<String> {
        if !settings.proxy_enabled {
            return None;
        }

        let host = settings.proxy_host.trim();
        let port = settings.proxy_port.trim();
        let number: i64 = port.parse().ok()?;
        if host.is_empty() || !(1..=65535).contains(&number) {
            return None;
        }

        let scheme = match settings.proxy_protocol {
            ProxyProtocol::Https => "https",
            ProxyProtocol::Socks5 => "socks5",
            ProxyProtocol::Http => "http",
        };

        Some(format!("{scheme}://{host}:{number}"))
    }

    /// True when the given URL should be downloaded through the proxy.
    pub fn should_use_proxy(settings: &AppSettings, url: Option<&str>) -> bool {
        if Self::build_proxy_url(settings).is_none() {
            return false;
        }

        let listed = url
            .filter(|value| !value.trim().is_empty())
            .is_some_and(|value| {
                settings
                    .proxy_sites
                    .iter()
                    .any(|site| Self::matches(value, site))
            });

        match settings.proxy_list_mode {
            ProxyListMode::Whitelist => listed,
            ProxyListMode::Blacklist => !listed,
        }
    }

    /// Checks whether a URL belongs to a configured website entry. Both the host
    /// name (with subdomain matching, ignoring a leading `www.`) and the path
    /// prefix are compared.
    pub fn matches(url: &str, site: &str) -> bool {
        if url.trim().is_empty() || site.trim().is_empty() {
            return false;
        }

        let (Some((url_host, url_path)), Some((site_host, site_path))) =
            (Self::try_parse(url), Self::try_parse(site))
        else {
            return url
                .trim_start()
                .to_lowercase()
                .starts_with(&site.trim().to_lowercase());
        };

        if !Self::host_matches(&url_host, &site_host) {
            return false;
        }

        // A path in the entry (anything beyond "/") narrows the rule to that prefix.
        site_path.is_empty()
            || site_path == "/"
            || url_path
                .to_lowercase()
                .starts_with(&site_path.to_lowercase())
    }

    fn host_matches(url_host: &str, site_host: &str) -> bool {
        let url = Self::normalize_host(url_host);
        let site = Self::normalize_host(site_host);

        url.eq_ignore_ascii_case(&site)
            || url
                .to_lowercase()
                .ends_with(&format!(".{}", site.to_lowercase()))
    }

    fn normalize_host(host: &str) -> String {
        if host
            .get(..4)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("www."))
        {
            host[4..].to_string()
        } else {
            host.to_string()
        }
    }

    /// Returns `(host, absolute_path)` when the value parses as an absolute URL.
    /// Bare hosts like `youtube.com` are retried with an `http://` prefix.
    fn try_parse(value: &str) -> Option<(String, String)> {
        let text = value.trim();
        let text = if text.contains("://") {
            text.to_string()
        } else {
            format!("http://{text}")
        };

        let uri = Url::parse(&text).ok()?;
        let host = uri.host_str()?.to_string();
        if host.is_empty() {
            return None;
        }

        Some((host, uri.path().to_string()))
    }
}
