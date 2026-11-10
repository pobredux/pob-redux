/**
 * Display translation for the stat rows Path of Building sends for the sidebar.
 *
 * PoB's calc modules emit the label text in English (`BuildDisplayStats.lua`).
 * Those labels are interface furniture rather than game data, so they are
 * translated here at render time; the engine keeps its own strings and every
 * lookup that keys off a label keeps working.
 *
 * Tables are one JSON file per language in `./tables/<locale>.json` and are
 * discovered at build time. Adding a language needs no code change. A label
 * missing from a table falls through as English rather than as a key.
 */
import { matchLocale } from "../locale-match";

const MODULES = import.meta.glob<Record<string, string>>("./tables/*.json", {
  eager: true,
  import: "default",
});

const TABLES: Record<string, Record<string, string>> = {};
for (const [file, table] of Object.entries(MODULES)) {
  const tag = file.match(/\/([^/]+)\.json$/)?.[1];
  if (tag) TABLES[tag] = table;
}
const LOCALES = Object.keys(TABLES);

/** One PoB colour code at the start of a string (`^7`, `^xRRGGBB`). */
const CODE = /^\^(?:x[0-9A-Fa-f]{6}|[0-9])/;

/**
 * Translate a PoB stat label for display. Colour codes are kept, a trailing
 * colon is dropped, and anything unknown falls through as the English text.
 */
export function statLabel(text: string | null | undefined, locale: string): string {
  if (!text) return "";
  const plain = text.replace(/:\s*$/, "");
  const tag = matchLocale(locale, LOCALES);
  const dict = tag ? TABLES[tag] : undefined;
  if (!dict) return plain;
  let codes = "";
  let rest = plain;
  for (;;) {
    const hit = CODE.exec(rest);
    if (!hit) break;
    codes += hit[0];
    rest = rest.slice(hit[0].length);
  }
  const translated = dict[rest];
  if (!translated) return plain;
  return `${codes}${translated}`;
}
