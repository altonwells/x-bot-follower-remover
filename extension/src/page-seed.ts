// This function is serialized by Chrome. Keep its helpers inside the function.
// HTML parsing is inert: no downloaded JavaScript is executed.
export async function readSigningPage() {
  const collect = (doc: Document, source: "document" | "html") => {
    const scripts = Array.from(
      doc.querySelectorAll<HTMLScriptElement>("script"),
    );
    return {
      source,
      scripts: [
        ...new Set([
          ...scripts.map((s) => s.getAttribute("src") ?? ""),
          ...Array.from(
            doc.querySelectorAll<HTMLLinkElement>('link[rel="modulepreload"]'),
            (s) => s.getAttribute("href") ?? "",
          ),
          ...(source === "document"
            ? performance.getEntriesByType("resource").map((r) => r.name)
            : []),
        ]),
      ]
        .map((u) => {
          try {
            return new URL(u, "https://x.com/home").href;
          } catch {
            return "";
          }
        })
        .filter(
          (u) =>
            /^https:\/\/abs\.twimg\.com\/(?:responsive-web|x-web)\//.test(u) &&
            /\.js(?:\?|$)/.test(u),
        )
        .slice(0, 200),
      inline: scripts
        .filter((s) => !s.hasAttribute("src"))
        .map((s) => s.textContent ?? "")
        .join("\n")
        .slice(0, 2_000_000),
      key:
        doc
          .querySelector('meta[name="twitter-site-verification"]')
          ?.getAttribute("content") ?? "",
      frames: Array.from(
        doc.querySelectorAll(
          'svg[id^="loading-x-anim"] g:first-child path:nth-child(2)',
        ),
        (p) => p.getAttribute("d") ?? "",
      ),
      error: "",
      code: "signing_unavailable",
      retryAt: null as number | null,
    };
  };
  const live = collect(document, "document");
  if (live.key && live.frames.length === 4) return live;
  // X can remove its loading animation after mounting. The original response
  // retains the SVGs and verification key from the same page build.
  try {
    const response = await fetch("https://x.com/home", {
      credentials: "include",
      cache: "no-store",
      redirect: "error",
      signal: AbortSignal.timeout(10_000),
    });
    if (!response.ok) {
      if (response.status === 429) {
        const reset = Number(response.headers.get("x-rate-limit-reset")) * 1000;
        return {
          ...live,
          error: "X home returned HTTP 429. Wait before retrying.",
          code: "rate_limited",
          retryAt: reset > Date.now() ? reset : Date.now() + 60_000,
        };
      }
      return {
        ...live,
        error: `X home returned HTTP ${response.status}. Open X and check your sign-in.`,
        code:
          response.status === 401 || response.status === 403
            ? "access_denied"
            : "signing_unavailable",
      };
    }
    const html = await response.text();
    if (html.length > 8_000_000)
      return { ...live, error: "X home exceeded the signing-page size limit." };
    return collect(new DOMParser().parseFromString(html, "text/html"), "html");
  } catch {
    return {
      ...live,
      error:
        "Could not read X's original home page (redirect, timeout, or network error). Open x.com/home and retry.",
    };
  }
}
