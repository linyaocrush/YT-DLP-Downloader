//! Ported from `YtDlpDownloader.Core.Tests/Models/ProxyRulesTests.cs`.
//!
//! The C# `BuildProxyUrl_NullSettings_ReturnsNull` case is dropped: Rust has no
//! nullable `&AppSettings`.

use ytdlp_downloader_core::{AppSettings, ProxyListMode, ProxyProtocol, ProxyRules};

fn enabled(protocol: ProxyProtocol) -> AppSettings {
    AppSettings {
        proxy_enabled: true,
        proxy_host: "127.0.0.1".to_string(),
        proxy_port: "7890".to_string(),
        proxy_protocol: protocol,
        ..Default::default()
    }
}

#[test]
fn build_proxy_url_disabled_returns_null() {
    let settings = AppSettings {
        proxy_enabled: false,
        ..enabled(ProxyProtocol::Http)
    };
    assert_eq!(ProxyRules::build_proxy_url(&settings), None);
}

#[test]
fn build_proxy_url_empty_host_returns_null() {
    for host in ["", "   "] {
        let settings = AppSettings {
            proxy_host: host.to_string(),
            ..enabled(ProxyProtocol::Http)
        };
        assert_eq!(ProxyRules::build_proxy_url(&settings), None);
    }
}

#[test]
fn build_proxy_url_invalid_port_returns_null() {
    for port in ["", "abc", "0", "65536", "-1"] {
        let settings = AppSettings {
            proxy_port: port.to_string(),
            ..enabled(ProxyProtocol::Http)
        };
        assert_eq!(ProxyRules::build_proxy_url(&settings), None);
    }
}

#[test]
fn build_proxy_url_protocol_maps_scheme() {
    assert_eq!(
        ProxyRules::build_proxy_url(&enabled(ProxyProtocol::Http)).as_deref(),
        Some("http://127.0.0.1:7890")
    );
    assert_eq!(
        ProxyRules::build_proxy_url(&enabled(ProxyProtocol::Https)).as_deref(),
        Some("https://127.0.0.1:7890")
    );
    assert_eq!(
        ProxyRules::build_proxy_url(&enabled(ProxyProtocol::Socks5)).as_deref(),
        Some("socks5://127.0.0.1:7890")
    );
}

#[test]
fn build_proxy_url_trims_host_and_port() {
    let settings = AppSettings {
        proxy_host: "  example.com  ".to_string(),
        proxy_port: " 1080 ".to_string(),
        ..enabled(ProxyProtocol::Http)
    };
    assert_eq!(
        ProxyRules::build_proxy_url(&settings).as_deref(),
        Some("http://example.com:1080")
    );
}

#[test]
fn should_use_proxy_disabled_returns_false() {
    let settings = AppSettings {
        proxy_enabled: false,
        proxy_sites: vec!["youtube.com".to_string()],
        ..enabled(ProxyProtocol::Http)
    };
    assert!(!ProxyRules::should_use_proxy(
        &settings,
        Some("https://youtube.com/x")
    ));
}

#[test]
fn should_use_proxy_whitelist_listed_url_true() {
    let settings = AppSettings {
        proxy_list_mode: ProxyListMode::Whitelist,
        proxy_sites: vec!["youtube.com".to_string()],
        ..enabled(ProxyProtocol::Http)
    };
    assert!(ProxyRules::should_use_proxy(
        &settings,
        Some("https://youtube.com/watch?v=1")
    ));
    assert!(!ProxyRules::should_use_proxy(
        &settings,
        Some("https://vimeo.com/1")
    ));
}

#[test]
fn should_use_proxy_blacklist_listed_url_false() {
    let settings = AppSettings {
        proxy_list_mode: ProxyListMode::Blacklist,
        proxy_sites: vec!["youtube.com".to_string()],
        ..enabled(ProxyProtocol::Http)
    };
    assert!(!ProxyRules::should_use_proxy(
        &settings,
        Some("https://youtube.com/watch?v=1")
    ));
    assert!(ProxyRules::should_use_proxy(
        &settings,
        Some("https://vimeo.com/1")
    ));
}

#[test]
fn should_use_proxy_null_url_whitelist_false_blacklist_true() {
    let mut settings = AppSettings {
        proxy_list_mode: ProxyListMode::Whitelist,
        proxy_sites: vec!["youtube.com".to_string()],
        ..enabled(ProxyProtocol::Http)
    };
    assert!(!ProxyRules::should_use_proxy(&settings, None));

    settings.proxy_list_mode = ProxyListMode::Blacklist;
    assert!(ProxyRules::should_use_proxy(&settings, None));
}

#[test]
fn matches_host_matching_ignores_www_and_allows_subdomains() {
    let cases = [
        ("https://youtube.com/watch", "youtube.com", true),
        ("https://www.youtube.com/watch", "youtube.com", true),
        ("https://m.youtube.com/watch", "youtube.com", true),
        ("https://youtube.com/watch", "www.youtube.com", true),
        ("https://notyoutube.com/watch", "youtube.com", false),
        ("https://youtube.com/watch", "vimeo.com", false),
    ];

    for (url, site, expected) in cases {
        assert_eq!(ProxyRules::matches(url, site), expected, "{url} vs {site}");
    }
}

#[test]
fn matches_path_prefix_narrows_rule() {
    let cases = [
        ("https://example.com/a/b", "https://example.com/a", true),
        ("https://example.com/b", "https://example.com/a", false),
        ("https://example.com", "https://example.com/", true),
    ];

    for (url, site, expected) in cases {
        assert_eq!(ProxyRules::matches(url, site), expected, "{url} vs {site}");
    }
}

#[test]
fn matches_site_without_scheme_parses() {
    assert!(ProxyRules::matches(
        "https://youtube.com/watch?v=1",
        "youtube.com/watch"
    ));
}

#[test]
fn matches_blank_inputs_return_false() {
    let cases = [
        ("", "youtube.com"),
        ("   ", "youtube.com"),
        ("https://youtube.com", ""),
        ("https://youtube.com", "   "),
    ];

    for (url, site) in cases {
        assert!(!ProxyRules::matches(url, site), "{url:?} vs {site:?}");
    }
}
