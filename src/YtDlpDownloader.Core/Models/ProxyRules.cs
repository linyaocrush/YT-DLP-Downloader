namespace YtDlpDownloader.Core.Models;

/// <summary>
/// Pure helper that turns the proxy settings into a yt-dlp --proxy value and decides whether a
/// given URL should be routed through the proxy (master switch + website list rules).
/// </summary>
public static class ProxyRules
{
    /// <summary>
    /// Builds the "scheme://host:port" value for yt-dlp, or null when the proxy is disabled or
    /// the host/port are invalid.
    /// </summary>
    public static string? BuildProxyUrl(AppSettings settings)
    {
        if (settings is null || !settings.ProxyEnabled)
            return null;

        var host = (settings.ProxyHost ?? string.Empty).Trim();
        var port = (settings.ProxyPort ?? string.Empty).Trim();
        if (host.Length == 0
            || !int.TryParse(port, out var number)
            || number is < 1 or > 65535)
        {
            return null;
        }

        var scheme = settings.ProxyProtocol switch
        {
            ProxyProtocol.Https => "https",
            ProxyProtocol.Socks5 => "socks5",
            _ => "http",
        };

        return $"{scheme}://{host}:{number}";
    }

    /// <summary>True when the given URL should be downloaded through the proxy.</summary>
    public static bool ShouldUseProxy(AppSettings settings, string? url)
    {
        if (BuildProxyUrl(settings) is null)
            return false;

        var sites = settings.ProxySites ?? new List<string>();
        var listed = !string.IsNullOrWhiteSpace(url)
                     && sites.Any(site => Matches(url, site));

        return settings.ProxyListMode == ProxyListMode.Whitelist ? listed : !listed;
    }

    /// <summary>
    /// Checks whether a URL belongs to a configured website entry. Both the host name (with
    /// subdomain matching, ignoring a leading "www.") and the path prefix are compared, so
    /// "https://www.youtube.com/" covers any page on youtube.com and its subdomains.
    /// </summary>
    public static bool Matches(string url, string site)
    {
        if (string.IsNullOrWhiteSpace(url) || string.IsNullOrWhiteSpace(site))
            return false;

        if (!TryParse(url, out var urlHost, out var urlPath)
            || !TryParse(site, out var siteHost, out var sitePath))
        {
            return url.TrimStart().StartsWith(site.Trim(), StringComparison.OrdinalIgnoreCase);
        }

        if (!HostMatches(urlHost, siteHost))
            return false;

        // A path in the entry (anything beyond "/") narrows the rule to that prefix.
        return string.IsNullOrEmpty(sitePath)
               || sitePath == "/"
               || urlPath.StartsWith(sitePath, StringComparison.OrdinalIgnoreCase);
    }

    private static bool HostMatches(string urlHost, string siteHost)
    {
        var url = NormalizeHost(urlHost);
        var site = NormalizeHost(siteHost);

        return string.Equals(url, site, StringComparison.OrdinalIgnoreCase)
               || url.EndsWith("." + site, StringComparison.OrdinalIgnoreCase);
    }

    private static string NormalizeHost(string host)
        => host.StartsWith("www.", StringComparison.OrdinalIgnoreCase) ? host[4..] : host;

    private static bool TryParse(string value, out string host, out string path)
    {
        host = string.Empty;
        path = string.Empty;

        var text = value.Trim();
        if (!text.Contains("://", StringComparison.Ordinal))
            text = "http://" + text;

        if (!Uri.TryCreate(text, UriKind.Absolute, out var uri))
            return false;

        host = uri.Host;
        path = uri.AbsolutePath;
        return host.Length > 0;
    }
}