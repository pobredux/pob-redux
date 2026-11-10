/**
 * Pure helpers for translating PoB "compare" tooltip lines. No framework
 * imports, so the exact production logic can be exercised against real engine
 * strings from a plain Node/Bun script.
 *
 * Modules/Build.lua:CompareStatList and Classes/CompareEntry.lua:CompareStatList
 * emit (^8 is an inline colour code, not part of the text):
 *
 *   <colour><signed value> <stat label> (<signed pct>) ^8[<value> (<pct>) per point]
 *
 * Only <stat label> is a dictionary key; everything else is generated at
 * runtime with the node's numbers. Translating the whole line can never hit.
 *
 * The decorations below are matched against the *engine's* output, which is
 * always English regardless of the UI language. They say nothing about the
 * target language: the replacement word comes from the active dictionary.
 */
export type Lookup = (s: string) => string | null;

const CODE_AT = /\^(?:x[0-9A-Fa-f]{6}|[0-9])/y;

/**
 * Drop PoB colour codes but remember the original index of every surviving
 * character, so a translated label can be spliced back in without losing codes
 * that sit inside the line (notably the ^8 on the "per point" bracket).
 */
export function stripCodes(s: string): { plain: string; map: number[] } {
  const map: number[] = [];
  const out: string[] = [];
  for (let i = 0; i < s.length; ) {
    if (s[i] === "^") {
      CODE_AT.lastIndex = i;
      if (CODE_AT.exec(s)) {
        i = CODE_AT.lastIndex;
        continue;
      }
    }
    map.push(i);
    out.push(s[i]);
    i++;
  }
  return { plain: out.join(""), map };
}

// The value has a forced sign; that is what keeps ordinary mod lines such as
// "10% increased Life" out of this path. Suffixes: %, s, m, and thousand separators.
const COMPARE_PREFIX = /^([+-][\d,.]+[%a-zA-Z]{0,3}\s+)(.+)$/;
// Both half-width and full-width parens occur in engine/user strings; the
// closing side may be either width too.
const COMPARE_PCT_TAIL = /[（(]\s*[+-][\d,.]+\s*%\s*[）)]\s*$/;
const COMPARE_BRACKET_TAIL = /\[[^\]]*per\s*point\]\s*$/i;
// Include the preceding space so "(-1.0%)per point]" and "(-1.0%) per point]"
// both end up with exactly one space before the translated word.
export const PER_POINT = /\s*per\s*point/gi;

export interface LabelSpan {
  start: number;
  end: number;
  translation: string;
}

/**
 * Find the exact substring holding the stat label inside a decode-free compare
 * line. Decorations are peeled from the end so nested "(±x%)" inside the
 * "per point" bracket is removed together with its bracket.
 */
export function findCompareLabel(plain: string, lookup: Lookup): LabelSpan | null {
  const m = COMPARE_PREFIX.exec(plain);
  if (!m) return null;
  const bodyStart = m[1].length;
  let limit = plain.length;
  for (;;) {
    const body = plain.slice(bodyStart, limit);
    const pct = COMPARE_PCT_TAIL.exec(body);
    if (pct) {
      limit = bodyStart + pct.index;
      continue;
    }
    const bracket = COMPARE_BRACKET_TAIL.exec(body);
    if (bracket) {
      limit = bodyStart + bracket.index;
      continue;
    }
    break;
  }
  const head = plain.slice(bodyStart, limit);
  const label = head.trim();
  if (!label) return null;
  const translation = lookup(label);
  if (!translation) return null;
  const start = bodyStart + (head.length - head.trimStart().length);
  return { start, end: start + label.length, translation };
}
