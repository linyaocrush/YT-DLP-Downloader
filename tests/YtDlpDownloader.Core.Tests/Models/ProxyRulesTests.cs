using YtDlpDownloader.Core.Models;

namespace YtDlpDownloader.Core.Tests.Models;

public class ProxyRulesTests
{
    private static AppSettings Enabled(ProxyProtocol protocol = ProxyProtocol.Http)
        => new()
        {
            ProxyEnabled = true,
            ProxyHost = "127.0.0.1",
            ProxyPort = "7890",
            ProxyProtocol = protocol,
        };

    [Fact]
    public void BuildProxyUrl_NullSettings_ReturnsNull()
        => Assert.Null(ProxyRules.BuildProxyUrl(null!));

    [Fact]
    public void BuildProxyUrl_Disabled_ReturnsNull()
    {
        var settings = Enabled();
        settings.ProxyEnabled = false;

        Assert.Null(ProxyRules.BuildProxyUrl(settings));
    }

    [Theory]
    [InlineData("")]
    [InlineData("   ")]
    [InlineData(null)]
    public void BuildProxyUrl_EmptyHost_ReturnsNull(string? host)
    {
        var settings = Enabled();
        settings.ProxyHost = host!;

        Assert.Null(ProxyRules.BuildProxyUrl(settings));
    }

    [Theory]
    [InlineData("")]
    [InlineData("abc")]
    [InlineData("0")]
    [InlineData("65536")]
    [InlineData("-1")]
    public void BuildProxyUrl_InvalidPort_ReturnsNull(string port)
    {
        var settings = Enabled();
        settings.ProxyPort = port;

        Assert.Null(ProxyRules.BuildProxyUrl(settings));
    }

    [Theory]
    [InlineData(ProxyProtocol.Http, "http://127.0.0.1:7890")]
    [InlineData(ProxyProtocol.Https, "https://127.0.0.1:7890")]
    [InlineData(ProxyProtocol.Socks5, "socks5://127.0.0.1:7890")]
    public void BuildProxyUrl_Protocol_MapsScheme(ProxyProtocol protocol, string expected)
        => Assert.Equal(expected, ProxyRules.BuildProxyUrl(Enabled(protocol)));

    [Fact]
    public void BuildProxyUrl_TrimsHostAndPort()
    {
        var settings = Enabled();
        settings.ProxyHost = "  example.com  ";
        settings.ProxyPort = " 1080 ";

        Assert.Equal("http://example.com:1080", ProxyRules.BuildProxyUrl(settings));
    }

    [Fact]
    public void ShouldUseProxy_Disabled_ReturnsFalse()
    {
        var settings = Enabled();
        settings.ProxyEnabled = false;
        settings.ProxySites.Add("youtube.com");

        Assert.False(ProxyRules.ShouldUseProxy(settings, "https://youtube.com/x"));
    }

    [Fact]
    public void ShouldUseProxy_Whitelist_ListedUrl_True()
    {
        var settings = Enabled();
        settings.ProxyListMode = ProxyListMode.Whitelist;
        settings.ProxySites.Add("youtube.com");

        Assert.True(ProxyRules.ShouldUseProxy(settings, "https://youtube.com/watch?v=1"));
        Assert.False(ProxyRules.ShouldUseProxy(settings, "https://vimeo.com/1"));
    }

    [Fact]
    public void ShouldUseProxy_Blacklist_ListedUrl_False()
    {
        var settings = Enabled();
        settings.ProxyListMode = ProxyListMode.Blacklist;
        settings.ProxySites.Add("youtube.com");

        Assert.False(ProxyRules.ShouldUseProxy(settings, "https://youtube.com/watch?v=1"));
        Assert.True(ProxyRules.ShouldUseProxy(settings, "https://vimeo.com/1"));
    }

    [Fact]
    public void ShouldUseProxy_NullUrl_WhitelistFalse_BlacklistTrue()
    {
        var settings = Enabled();
        settings.ProxyListMode = ProxyListMode.Whitelist;
        settings.ProxySites.Add("youtube.com");
        Assert.False(ProxyRules.ShouldUseProxy(settings, null));

        settings.ProxyListMode = ProxyListMode.Blacklist;
        Assert.True(ProxyRules.ShouldUseProxy(settings, null));
    }

    [Theory]
    [InlineData("https://youtube.com/watch", "youtube.com", true)]
    [InlineData("https://www.youtube.com/watch", "youtube.com", true)]
    [InlineData("https://m.youtube.com/watch", "youtube.com", true)]
    [InlineData("https://youtube.com/watch", "www.youtube.com", true)]
    [InlineData("https://notyoutube.com/watch", "youtube.com", false)]
    [InlineData("https://youtube.com/watch", "vimeo.com", false)]
    public void Matches_HostMatching_IgnoresWwwAndAllowsSubdomains(
        string url, string site, bool expected)
        => Assert.Equal(expected, ProxyRules.Matches(url, site));

    [Theory]
    [InlineData("https://example.com/a/b", "https://example.com/a", true)]
    [InlineData("https://example.com/b", "https://example.com/a", false)]
    [InlineData("https://example.com", "https://example.com/", true)]
    public void Matches_PathPrefix_NarrowsRule(string url, string site, bool expected)
        => Assert.Equal(expected, ProxyRules.Matches(url, site));

    [Fact]
    public void Matches_SiteWithoutScheme_Parses()
        => Assert.True(ProxyRules.Matches("https://youtube.com/watch?v=1", "youtube.com/watch"));

    [Theory]
    [InlineData("", "youtube.com")]
    [InlineData("   ", "youtube.com")]
    [InlineData("https://youtube.com", "")]
    [InlineData("https://youtube.com", "   ")]
    public void Matches_BlankInputs_ReturnFalse(string url, string site)
        => Assert.False(ProxyRules.Matches(url, site));
}
