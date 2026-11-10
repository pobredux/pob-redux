/**
 * Language-neutral translation core for Path of Building display text.
 *
 * Dictionaries are keyed on the *English* strings PoB emits. Nothing in the
 * vendored data or the engine is changed, so item parsing, mod matching, build
 * XML and every lookup that uses a name as a key keep working. This module is
 * only called at render points -- never on a value fed back into the engine or
 * compared with a literal (CalcsView, for one, keys off row labels).
 *
 * There are no framework or bundler imports here, so verification scripts can
 * run the exact production logic under plain Node/Bun.
 */
import { stripCodes, findCompareLabel, PER_POINT, type Lookup } from "./compare-line";

export type { Lookup };

export interface GameDict {
  meta: {
    source?: string;
    locale?: string;
    game?: string;
    scope?: string;
    exact?: number;
    patterns?: number;
    [key: string]: unknown;
  };
  exact: Record<string, string>;
  patterns: Record<string, string>;
}

const LEADING_CODES = /^(?:\^(?:x[0-9A-Fa-f]{6}|[0-9]))+/;
const NUMBER = /\d+(?:\.\d+)?/g;
const PLACEHOLDER = /\{(\d+)\}/g;

/** Dictionary key for the trailing decoration the engine writes as "per point". */
export const PER_POINT_LABEL = "Per Point";

/** Replace numbers with {0}, {1}, ... the same way the generator keys templates. */
export function canonicalize(s: string): { key: string; numbers: string[] } {
  const numbers: string[] = [];
  let i = 0;
  const key = s.replace(NUMBER, (n) => {
    numbers.push(n);
    return `{${i++}}`;
  });
  return { key, numbers };
}

export function lookupIn(dict: GameDict, s: string): string | null {
  const hit = dict.exact[s];
  if (hit) return hit;
  const { key, numbers } = canonicalize(s);
  // "+30% ..." against a "{0}% ..." template: the sign is not part of the
  // number, so put it back or the translation silently loses the "+" bonus.
  let template = dict.patterns[key];
  let sign = "";
  if (!template && key.startsWith("+{")) {
    template = dict.patterns[key.slice(1)];
    sign = "+";
  }
  if (!template) return null;
  return sign + template.replace(PLACEHOLDER, (_, i: string) => numbers[Number(i)] ?? "");
}

/** Interface labels take precedence over game-object tables; first hit wins. */
export function lookupMany(dicts: readonly (GameDict | null | undefined)[], s: string): string | null {
  for (const dict of dicts) {
    if (!dict) continue;
    const hit = lookupIn(dict, s);
    if (hit) return hit;
  }
  return null;
}

/** Whole-line dictionary hit: headers, mod descriptions, standalone labels. */
function translatePlain(s: string, lookup: Lookup): string | null {
  const core = s.trim();
  if (!core) return null;
  let hit = lookup(core);
  if (!hit) {
    const withoutColon = core.replace(/[:：]\s*$/, "");
    if (withoutColon !== core) hit = lookup(withoutColon);
  }
  if (!hit) return null;
  // Function replacement: a translation containing "$&" must stay literal.
  return s.replace(core, () => hit);
}

/**
 * Translate one PoB display line with the active language's lookup.
 * Compare tooltip lines are special: only their stat label is a dictionary key,
 * so it is replaced in place and every colour code, signed value, percent
 * change and "per point" bracket survives intact. A line with no translation
 * is returned byte-for-byte, which is how languages without dictionaries keep
 * the engine's original wording.
 */
export function translateGameText(text: string | null | undefined, lookup: Lookup): string {
  if (!text) return "";
  const codes = LEADING_CODES.exec(text)?.[0] ?? "";
  const rest = text.slice(codes.length);
  const { plain, map } = stripCodes(rest);

  // First try the whole line. A hit here drops inline codes, whose positions
  // refer to the source text and no longer mean anything after translation.
  const whole = translatePlain(plain, lookup);
  if (whole) return codes + whole;

  const span = findCompareLabel(plain, lookup);
  if (!span) return text;
  const perPoint = lookup(PER_POINT_LABEL);
  const head = rest.slice(0, map[span.start]);
  let tail = rest.slice(map[span.end - 1] + 1);
  if (perPoint) tail = tail.replace(PER_POINT, () => ` ${perPoint}`);
  return codes + head + span.translation + tail;
}
