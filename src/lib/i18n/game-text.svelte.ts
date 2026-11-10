/**
 * Reactive shell around the language-neutral translation core in game-dict.ts.
 *
 * It picks the dictionaries for the active UI locale and game, loads each one
 * lazily, and bumps a revision when it arrives so templates that called
 * `gameText()` re-render. A locale with no dictionary (or a failed load) keeps
 * PoB's original wording. Nothing here is language-specific: see
 * dictionaries.ts for how a new language is added.
 */
import { locale } from "$lib/state/locale.svelte";
import { game } from "$lib/state/game.svelte";
import { translateGameText, lookupMany, type GameDict, type Lookup } from "./game-dict";
import { dictionaryLoaders } from "./dictionaries";

// Plain Maps, not $state: proxying 67k-entry objects would be wasteful.
const loaded = new Map<string, GameDict>();
const pending = new Map<string, Promise<void>>();
const failed = new Set<string>();
// Bumped when a dictionary arrives so templates that called gameText() re-run.
let revision = $state(0);

// gameText() runs thousands of times while a tree or calc page renders. Rebuild
// the lookup only when the language, game or loaded dictionaries change.
let cachedKey = "";
let cachedRevision = -1;
let cachedLookup: Lookup | null = null;

/** Start any missing loads and return a lookup over the dictionaries ready now. */
function lookupFor(): Lookup | null {
  const language = locale.current;
  const activeGame = game.current;
  const requests = dictionaryLoaders(language, activeGame);
  if (!requests.length) return null;
  for (const request of requests) {
    const id = request.id;
    if (loaded.has(id) || pending.has(id) || failed.has(id)) continue;
    const task = request
      .load()
      .then((dict) => {
        loaded.set(id, dict);
        revision++;
      })
      .catch((error) => {
        console.warn(`game text dictionary ${id} failed to load; leaving text untranslated.`, error);
        failed.add(id);
      })
      .finally(() => pending.delete(id));
    pending.set(id, task);
  }
  const key = `${language}/${activeGame}`;
  if (cachedKey === key && cachedRevision === revision) return cachedLookup;
  const dicts = requests.map((request) => loaded.get(request.id));
  cachedKey = key;
  cachedRevision = revision;
  cachedLookup = dicts.some(Boolean) ? (s) => lookupMany(dicts, s) : null;
  return cachedLookup;
}

/** Translate a PoB display string for the current UI language. */
export function gameText(text: string | null | undefined): string {
  void revision; // re-render when a dictionary finishes loading
  if (!text) return "";
  const lookup = lookupFor();
  if (!lookup) return text;
  return translateGameText(text, lookup);
}
