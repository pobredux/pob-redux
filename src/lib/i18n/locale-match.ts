/**
 * Resolve the UI's locale against the locales that ship a data file.
 *
 * UI locales come from Paraglide (`project.inlang/settings.json`); translation
 * tables may be shipped at a different granularity. Exact match wins, then the
 * bare language, then the first locale sharing that language. A language with
 * no table returns null and its UI simply shows PoB's original English text.
 */
export function matchLocale(requested: string, available: readonly string[]): string | null {
  const lower = requested.toLowerCase();
  const exact = available.find((tag) => tag.toLowerCase() === lower);
  if (exact) return exact;
  const base = lower.split("-")[0];
  return (
    available.find((tag) => tag.toLowerCase() === base) ??
    available.find((tag) => tag.toLowerCase().split("-")[0] === base) ??
    null
  );
}
