/**
 * Build-time discovery of display dictionaries. Adding a language is purely a
 * data change: drop `ui.json` and `<game>.json` into
 * `src/lib/i18n/data/locales/<locale>/` and they show up here automatically.
 *
 *   ui.json        interface labels, shared by every game;
 *   <game>.json    game objects, loaded only while that game is active.
 *
 * Files stay behind independent dynamic imports, so a language nobody selects
 * is never fetched and the large game-data tables remain their own chunks.
 */
import { matchLocale } from "./locale-match";
import type { GameDict } from "./game-dict";

type Loader = () => Promise<GameDict>;

const LOADERS = import.meta.glob<GameDict>("./data/locales/*/*.json", { import: "default" });
const PATH = /^\.\/data\/locales\/([^/]+)\/([^/]+)\.json$/;

/** locale -> scope -> lazy loader */
const INDEX = new Map<string, Map<string, Loader>>();
for (const [file, load] of Object.entries(LOADERS)) {
  const parts = PATH.exec(file);
  if (!parts) continue;
  const [, locale, scope] = parts;
  let scopes = INDEX.get(locale);
  if (!scopes) INDEX.set(locale, (scopes = new Map()));
  scopes.set(scope, load);
}

export interface DictionaryLoader {
  /** Stable cache key of the form "<locale>/<scope>". */
  id: string;
  scope: string;
  load: Loader;
}

/** Locale tags that ship at least one dictionary. */
export function dictionaryLocales(): string[] {
  return [...INDEX.keys()];
}

/** The dictionaries relevant right now, in lookup order (UI first). */
export function dictionaryLoaders(locale: string, game: string): DictionaryLoader[] {
  const tag = matchLocale(locale, dictionaryLocales());
  if (!tag) return [];
  const scopes = INDEX.get(tag);
  if (!scopes) return [];
  return ["ui", game]
    .filter((scope) => scopes.has(scope))
    .map((scope) => ({ id: `${tag}/${scope}`, scope, load: scopes.get(scope)! }));
}
