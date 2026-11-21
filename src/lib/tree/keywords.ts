import { readTreeKeywords } from "$lib/engine.svelte";

export type Keyword = { term: string; definition: string };
export type TreeKeywords = { lines: Record<string, string>; keywords: Record<string, Keyword> };
export type Segment = { text: string; id?: string };

const cache = new Map<string, Promise<TreeKeywords | null>>();

export function loadKeywords(version: string): Promise<TreeKeywords | null> {
  let loading = cache.get(version);
  if (!loading) {
    loading = readTreeKeywords(version)
      .then((text) => (text ? (JSON.parse(text) as TreeKeywords) : null))
      .catch(() => null);
    cache.set(version, loading);
  }
  return loading;
}

const LINK = /\[([A-Za-z0-9_]+)(?:\|([^\]]+))?\]/g;

export function segments(text: string, keywords: Record<string, Keyword>): Segment[] {
  const out: Segment[] = [];
  const plain = (t: string) => {
    if (!t) return;
    const prev = out.at(-1);
    if (prev && !prev.id) prev.text += t;
    else out.push({ text: t });
  };
  let last = 0;
  for (const match of text.matchAll(LINK)) {
    plain(text.slice(last, match.index));
    const shown = match[2] ?? match[1];
    if (keywords[match[1]]) out.push({ text: shown, id: match[1] });
    else plain(shown);
    last = match.index + match[0].length;
  }
  plain(text.slice(last));
  return out;
}
