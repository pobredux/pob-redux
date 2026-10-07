/**
 * The subset of markdown a model reply uses, parsed into blocks the panel
 * renders as Svelte markup. Nothing here produces HTML strings: model output
 * is untrusted, so it only ever becomes text nodes.
 */

import type { GemMatch } from "$lib/state/gems.svelte";

export type Span =
  | { kind: "text" | "bold" | "code"; text: string }
  | { kind: "gem"; text: string; gem: GemMatch }
  | { kind: "link"; text: string; href: string };

/** Splits plain text into runs, some of which name a gem. Set by the panel once the gem index has loaded. */
export type GemScanner = (text: string) => Array<{ text: string; gem?: GemMatch }>;

export type Block =
  | { kind: "p"; spans: Span[] }
  | { kind: "h"; spans: Span[] }
  | { kind: "list"; ordered: boolean; items: Span[][] }
  | { kind: "table"; head: Span[][]; rows: Span[][][] }
  | { kind: "pre"; text: string };

const NUMERIC = /^[\s\d.,%+\-/×x~<>=]+$/;

/** Cells that are only numbers render in the mono font, as every other number in the app. */
export function isNumeric(spans: Span[]): boolean {
  const text = spans.map((s) => s.text).join("").trim();
  return text.length > 0 && NUMERIC.test(text) && /\d/.test(text);
}

export function inline(text: string, scan?: GemScanner): Span[] {
  const out: Span[] = [];
  const plain = (t: string) => {
    if (!scan) {
      out.push({ kind: "text", text: t });
      return;
    }
    for (const run of scan(t)) {
      if (run.gem) out.push({ kind: "gem", text: run.text, gem: run.gem });
      else out.push({ kind: "text", text: run.text });
    }
  };
  const re = /(\[([^\]\n]+)\]\((https?:\/\/[^\s)]+)\)|\*\*(.+?)\*\*|`([^`]+)`|(https?:\/\/[^\s<>"'`]+))/g;
  let last = 0;
  for (const m of text.matchAll(re)) {
    const at = m.index ?? 0;
    if (at > last) plain(text.slice(last, at));
    if (m[3] !== undefined) out.push({ kind: "link", text: m[2], href: m[3] });
    else if (m[4] !== undefined) {
      // A bold gem name is still a gem name.
      const inner = scan ? scan(m[4]) : [{ text: m[4] }];
      const gem = inner.length === 1 ? inner[0].gem : undefined;
      if (gem) out.push({ kind: "gem", text: m[4], gem });
      else out.push({ kind: "bold", text: m[4] });
    } else if (m[5] !== undefined) out.push({ kind: "code", text: m[5] });
    else if (m[6] !== undefined) {
      const href = m[6].replace(/[.,;:!?)\]]+$/, "");
      out.push({ kind: "link", text: "", href });
      if (href.length < m[6].length) plain(m[6].slice(href.length));
    }
    last = at + m[0].length;
  }
  if (last < text.length) plain(text.slice(last));
  return out;
}

export function tradeLeague(href: string): string | null {
  try {
    const url = new URL(href);
    if (!url.hostname.endsWith("pathofexile.com")) return null;
    const hit = /^\/trade2?\/search\/(?:(?:poe2|xbox|sony)\/)?([^/?#]+)/.exec(url.pathname);
    return hit ? decodeURIComponent(hit[1]) : null;
  } catch {
    return null;
  }
}

export function shortUrl(href: string): string {
  try {
    const url = new URL(href);
    const text = url.hostname.replace(/^www\./, "") + url.pathname.replace(/\/$/, "");
    return text.length > 40 ? `${text.slice(0, 39)}…` : text;
  } catch {
    return href;
  }
}

function cells(line: string, scan?: GemScanner): Span[][] {
  const trimmed = line.trim().replace(/^\|/, "").replace(/\|$/, "");
  return trimmed.split("|").map((c) => inline(c.trim(), scan));
}

const isTableLine = (l: string) => /^\s*\|.*\|\s*$/.test(l);
const isSeparator = (l: string) => /^\s*\|?\s*:?-{2,}:?\s*(\|\s*:?-{2,}:?\s*)*\|?\s*$/.test(l);

export function parse(text: string, scan?: GemScanner): Block[] {
  const lines = text.replace(/\r\n?/g, "\n").split("\n");
  const blocks: Block[] = [];
  let para: string[] = [];
  const flush = () => {
    if (para.length) {
      blocks.push({ kind: "p", spans: inline(para.join(" "), scan) });
      para = [];
    }
  };
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (line.trim() === "") {
      flush();
      continue;
    }
    if (line.startsWith("```")) {
      flush();
      const body: string[] = [];
      i++;
      while (i < lines.length && !lines[i].startsWith("```")) body.push(lines[i++]);
      blocks.push({ kind: "pre", text: body.join("\n") });
      continue;
    }
    const heading = /^#{1,6}\s+(.*)$/.exec(line);
    if (heading) {
      flush();
      blocks.push({ kind: "h", spans: inline(heading[1].trim(), scan) });
      continue;
    }
    if (isTableLine(line)) {
      flush();
      const head = cells(line, scan);
      const rows: Span[][][] = [];
      let j = i + 1;
      if (j < lines.length && isSeparator(lines[j])) j++;
      while (j < lines.length && isTableLine(lines[j])) {
        if (!isSeparator(lines[j])) rows.push(cells(lines[j], scan));
        j++;
      }
      blocks.push({ kind: "table", head, rows });
      i = j - 1;
      continue;
    }
    const item = /^\s*(?:[-*•]|\d+[.)])\s+(.*)$/.exec(line);
    if (item) {
      flush();
      const ordered = /^\s*\d/.test(line);
      const items: Span[][] = [inline(item[1], scan)];
      let j = i + 1;
      while (j < lines.length) {
        const next = /^\s*(?:[-*•]|\d+[.)])\s+(.*)$/.exec(lines[j]);
        if (next) items.push(inline(next[1], scan));
        else if (/^\s{2,}\S/.test(lines[j]) && items.length) items[items.length - 1].push(...inline(" " + lines[j].trim(), scan));
        else break;
        j++;
      }
      blocks.push({ kind: "list", ordered, items });
      i = j - 1;
      continue;
    }
    para.push(line.trim());
  }
  flush();
  return blocks;
}
