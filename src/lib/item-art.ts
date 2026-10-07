import { convertFileSrc } from "@tauri-apps/api/core";
import type { Tooltip } from "./engine.svelte";

type ArtItem = NonNullable<Tooltip["itemArt"]>;
type Game = ArtItem["game"];

export interface ArtMap {
  game: Game;
  version: string;
  bases: Record<string, string>;
  uniques: Record<string, string>;
  sockets: Record<string, string>;
  skills?: Record<string, string>;
  buffs?: Record<string, string>;
  buffNames?: Record<string, string>;
  buffVisuals?: Record<string, string>;
  classIcons?: Record<string, string>;
  files: Record<string, string>;
}

const lookup = (table: Record<string, string> | null | undefined, name: string | null | undefined) =>
  name && table && Object.hasOwn(table, name) ? table[name] : undefined;

export function artPath(map: ArtMap, item: ArtItem, supportGem = false): string | null {
  // PoB drops "Support" from PoE1 support gem names; some of them also name an active gem.
  const baseName = supportGem && item.game === "poe1" && item.baseName && !item.baseName.endsWith(" Support")
    ? `${item.baseName} Support`
    : item.baseName;
  const unique = item.rarity === "UNIQUE" || item.rarity === "RELIC";
  const candidates: [Record<string, string>, string | null | undefined][] = [];
  if (unique) candidates.push([map.uniques, item.name], [map.uniques, item.name?.replace(/^Foulborn\s+/, "")]);
  candidates.push([map.bases, baseName], [map.bases, baseName?.replace(/\s+\([^)]*\)$/, "")]);
  for (const [table, name] of candidates) {
    const path = lookup(table, name);
    if (path) return path;
  }
  return null;
}

/** Game inventory art is exported at 5/3 of the size PoE displays it. */
export function tooltipArtSize(naturalWidth: number, naturalHeight: number) {
  const scale = 3 / 5;
  return { width: Math.round(naturalWidth * scale), height: Math.round(naturalHeight * scale) };
}

const maps = new Map<Game, Promise<ArtMap | null>>();

function artMap(game: Game): Promise<ArtMap | null> {
  let pending = maps.get(game);
  if (!pending) {
    pending = fetch(convertFileSrc(`maps/${game}.json`, "pobart"))
      .then((res) => (res.ok ? (res.json() as Promise<ArtMap>) : null))
      .catch(() => null);
    maps.set(game, pending);
    pending.then((map) => {
      if (!map) maps.delete(game);
    });
  }
  return pending;
}

function artUrl(map: ArtMap, path: string): string | null {
  const tag = map.files[path];
  return tag ? convertFileSrc(`${map.game}/${tag}/${path}`, "pobart") : null;
}

const recipes = new Map<string, Promise<{ name: string; url: string | null }>>();

/** Tree recipes name PoE1 oils as "GoldenOil" and PoE2 emotions by the bare emotion ("Greed" is Diluted Liquid Greed). */
export function recipeArt(game: Game, word: string): Promise<{ name: string; url: string | null }> {
  const key = `${game}:${word}`;
  let pending = recipes.get(key);
  if (!pending) {
    pending = artMap(game).then((map) => {
      const bases = map?.bases ?? {};
      const emotions = Object.keys(bases).filter((k) => k === `Liquid ${word}` || k.endsWith(` Liquid ${word}`));
      const name =
        game === "poe1"
          ? word.replace(/Oil$/, " Oil")
          : (emotions.find((k) => !k.startsWith("Ancient ")) ?? emotions[0] ?? `Liquid ${word}`);
      const path = lookup(bases, name);
      if (!map) recipes.delete(key);
      return { name, url: map && path ? artUrl(map, path) : null };
    });
    recipes.set(key, pending);
  }
  return pending;
}

export async function itemArtUrl(item: ArtItem, supportGem = false): Promise<string | null> {
  const map = await artMap(item.game);
  const path = map && artPath(map, item, supportGem);
  return map && path ? artUrl(map, path) : null;
}

interface PrefetchItem {
  name: string;
  title?: string | null;
  baseName?: string | null;
  rarity?: string | null;
  runes?: string[];
}

export async function prefetchArt(game: Game, items: PrefetchItem[], gems: { name: string; support: boolean }[] = []) {
  const map = await artMap(game);
  if (!map) return;
  const urls = new Set<string>();
  const add = (item: ArtItem, supportGem = false) => {
    const path = artPath(map, item, supportGem);
    const url = path && artUrl(map, path);
    if (url) urls.add(url);
  };
  for (const item of items) {
    add({ game, name: item.title ?? item.name, baseName: item.baseName ?? null, rarity: item.rarity ?? null });
    for (const rune of item.runes ?? []) if (rune !== "None") add({ game, name: rune, baseName: rune, rarity: null });
  }
  for (const gem of gems) add({ game, name: gem.name, baseName: gem.name, rarity: null }, gem.support);
  for (const path of Object.values(map.sockets)) {
    const url = artUrl(map, path);
    if (url) urls.add(url);
  }
  await Promise.allSettled([...urls].map((url) => fetch(url)));
}

export async function socketArtUrls(game: Game): Promise<Record<string, string>> {
  const map = await artMap(game);
  if (!map) return {};
  return Object.fromEntries(
    Object.entries(map.sockets).flatMap(([key, path]) => {
      const url = artUrl(map, path);
      return url ? [[key, url]] : [];
    }),
  );
}

// The game names these Icon<attributes>_<ascendancy id>, and a class's own icon is the part before the underscore.
export async function classIconUrl(game: Game, ascendancyIds: string[], classAscendancyIds: string[]): Promise<string | null> {
  const map = await artMap(game);
  const icons = map?.classIcons;
  if (!map || !icons) return null;
  const keys = Object.keys(icons);
  const keyFor = (id: string) => keys.find((k) => k.endsWith(`_${id}`));
  for (const id of ascendancyIds) {
    const key = keyFor(id);
    if (key) return artUrl(map, icons[key]!);
  }
  for (const id of classAscendancyIds) {
    const key = keyFor(id);
    const prefix = key?.slice(0, key.lastIndexOf("_"));
    if (prefix && icons[prefix]) return artUrl(map, icons[prefix]!);
  }
  return null;
}

const looseKey = (name: string) => name.toLowerCase().replace(/[^a-z0-9]+/g, "");
const looseIndexes = new WeakMap<ArtMap, Record<string, string>>();

function looseIndex(map: ArtMap) {
  let index = looseIndexes.get(map);
  if (!index) {
    index = {};
    for (const table of [map.buffNames, map.skills]) {
      for (const [name, path] of Object.entries(table ?? {})) index[looseKey(name)] ??= path;
    }
    looseIndexes.set(map, index);
  }
  return index;
}

/** Resolve status art by name, stable id, then HUD skill art, then looser spellings of the name. */
export function statusArtPath(map: ArtMap, name: string): string | undefined {
  // PoB prefixes some buffs, as in "Totem Wrath", "Load Explosive Shot" and "Lesser Brutal Shrine".
  const base = name.replace(/^(?:Totem|Load|Lesser)\s+/, "");
  const names = [...new Set([name, base, base.endsWith("s") ? base.slice(0, -1) : `${base}s`])];
  for (const candidate of names) {
    const id = candidate.toLowerCase().replace(/[^a-z0-9]+/g, "_").replace(/^_|_$/g, "");
    const path = lookup(map.buffNames, candidate)
      ?? lookup(map.buffs, id)
      ?? lookup(map.buffVisuals, id)
      ?? lookup(map.skills, candidate)
      ?? lookup(looseIndex(map), looseKey(candidate));
    if (path) return path;
  }
  return undefined;
}

export async function statusEffectArtUrls(game: Game, names: string[]): Promise<Record<string, string>> {
  const map = await artMap(game);
  if (!map) return {};
  const urls: [string, string][] = [];
  for (const name of names) {
    const path = statusArtPath(map, name);
    const url = path && artUrl(map, path);
    if (url) urls.push([name, url]);
  }
  return Object.fromEntries(urls);
}
