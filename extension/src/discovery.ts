// Only static text is inspected. Downloaded X JavaScript is never evaluated.
export function assetURL(raw: string, base?: string): string | null {
  try {
    const url = new URL(raw, base);
    return url.origin === "https://abs.twimg.com" &&
      /^\/(?:responsive-web|x-web)\//.test(url.pathname) &&
      url.pathname.endsWith(".js")
      ? url.href
      : null;
  } catch {
    return null;
  }
}
export function scriptReferences(text: string, base: string): string[] {
  const found = new Set<string>();
  for (const m of text.matchAll(
    /["'`]((?:https:\/\/abs\.twimg\.com\/|\.{1,2}\/)[^"'`\s]+\.js(?:\?[^"'`\s]*)?)["'`]/g,
  )) {
    const url = assetURL(m[1], base);
    if (url) found.add(url);
  }
  // Vite can use bare relative chunk names, without a ./ prefix.
  for (const m of text.matchAll(
    /["'`]((?:(?:\.{0,2}\/)?[\w./-]*\/)?(?:ondemand\.s|sign\.o)[\w.-]*\.js(?:\?[^"'`\s]*)?)["'`]/g,
  )) {
    const url = assetURL(m[1], base);
    if (url) found.add(url);
  }
  // Legacy webpack's runtime pairs chunk names with seven- or sixteen-digit hashes.
  const names = new Map<string, string>(),
    hashes = new Map<string, string>();
  for (const m of text.matchAll(/(?:"(\d+)"|(\d+)):\s*"([\w.~/-]+)"/g)) {
    const id = m[1] ?? m[2],
      value = m[3];
    if (/^(?:[a-f0-9]{7}|[a-f0-9]{16})$/.test(value)) hashes.set(id, value);
    else names.set(id, value);
  }
  for (const [id, hash] of hashes) {
    const name = names.get(id) ?? id;
    const url = assetURL(
      `https://abs.twimg.com/responsive-web/client-web/${name}.${hash}a.js`,
    );
    if (url) found.add(url);
  }
  return [...found];
}
export function signingAsset(url: string): boolean {
  return /\/(?:ondemand\.s[.-]|sign\.o[.-])/.test(url);
}
export function assetPriority(url: string): number {
  return signingAsset(url)
    ? 0
    : /\/main\.|entry-client|\/app[.-]/.test(url)
      ? 1
      : /vendor|i18n|icons|syntax/.test(url)
        ? 3
        : 2;
}
