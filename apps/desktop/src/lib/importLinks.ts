export function linkKey(raw: string): string | null {
  try {
    const url = new URL(raw.trim());
    if (!["http:", "https:"].includes(url.protocol)) return null;
    const host = url.hostname.toLowerCase().replace(/^www\./, "");
    if (host === "youtu.be") return `youtube:${url.pathname.split("/")[1]}`;
    if (host === "youtube.com" || host.endsWith(".youtube.com")) {
      const id = url.searchParams.get("v") ?? url.pathname.match(/^\/(?:shorts|live|embed)\/([^/]+)/)?.[1];
      if (id && !url.searchParams.has("list")) return `youtube:${id}`;
    }
    url.hash = "";
    for (const key of [...url.searchParams.keys()]) if (key.startsWith("utm_") || ["si", "feature"].includes(key)) url.searchParams.delete(key);
    url.searchParams.sort();
    return `${host}${url.pathname.replace(/\/$/, "")}${url.search}`;
  } catch { return null; }
}

export function previewLinks(text: string, existing: string[]) {
  const keys = new Set(existing.map(linkKey).filter(Boolean));
  return text.split(/\r?\n/).map(url => url.trim()).filter(Boolean).map(url => {
    const key = linkKey(url);
    const status = !key ? "invalid" : keys.has(key) ? "duplicate" : "valid";
    if (key) keys.add(key);
    return {url, status};
  });
}
