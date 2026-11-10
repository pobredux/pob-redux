// Compiles a PoB translation CSV collection into the runtime display
// dictionaries consumed by src/lib/i18n/game-text.svelte.ts.
//
//   node scripts/build-game-dictionary.mjs [options]
//
//   --locale <tag>         output language tag, default "zh-CN"
//   --game <poe1|poe2>     which game's objects the CSVs describe, default "poe1"
//   --charm <dir>          root of a PoeCharm-style checkout; defaults to
//                          $POECHARM_DIR, else the sibling "PoeCharm-main"
//   --translate-dir <name> CSV folder under <charm>/Data/Translate; defaults
//                          to SOURCE_DIRS[locale] or the locale itself
//   --source <dir>         CSV folder directly, overrides --charm/--translate-dir
//   --out-dir <dir>        defaults to src/lib/i18n/data/locales/<locale>
//   --overrides <file>     defaults to src/lib/i18n/data/overrides/<locale>.json
//   --no-overrides         build the baseline without corrections (used when
//                          generating a locale variant, so gap filling can see
//                          what the upstream wordlist actually provides)
//   --bridge-from <locale> also index the entries of an already-built locale
//                          under that locale's translated text. Some vendored
//                          PoB data ships translated names rather than English
//                          keys; without the bridge those strings would stay in
//                          the other language when this locale is selected.
//
// Adding a language is a data operation, not a code change: put its two JSON
// files in src/lib/i18n/data/locales/<locale>/ and they are discovered at
// runtime by src/lib/i18n/dictionaries.ts. The same applies to sidebar labels:
// drop a JSON map in src/lib/i18n/stat-labels/tables/<locale>.json.
//
// Two JSON files are written per language:
//
//   ui.json        UI chrome (calc labels, tabs, config options). Game-agnostic,
//                  so it is used for every game.
//   <game>.json    Game objects (items, gems, mods, passive tree). Only loaded
//                  for that game: translations from one game's tables must not
//                  be applied to a different game's mechanics.
//
// Each file holds two kinds of key:
//
//   exact    concrete strings looked up as-is;
//   patterns strings with sequential {0}, {1} ... placeholders. At runtime a
//            rendered line has its numbers replaced with the same placeholders
//            to find the template, then the captured numbers are put back.
//
// Translation sources often key UI labels with a trailing colon
// ("Player modifiers:") while PoB emits the label without one. Both spellings
// are emitted, with the colon dropped from the translation as the surrounding
// UI supplies its own.
//
// Templates with repeated/out-of-order placeholders or literal digits are
// dropped: they cannot be round-tripped safely and would print wrong numbers.
// The per-locale overrides file is merged last, so bad source rows can be
// corrected without editing the upstream CSVs.
//
// This is display-only. Nothing here is written into the vendored PoB data, so
// item parsing, mod matching and build XML stay on the English keys.

import fs from "node:fs";
import path from "node:path";

const args = process.argv.slice(2);
const argOf = (flag, fallback) => {
  const i = args.indexOf(flag);
  return i >= 0 && args[i + 1] ? args[i + 1] : fallback;
};

const repo = path.resolve(import.meta.dirname, "..");
const locale = argOf("--locale", "zh-CN");
const game = argOf("--game", "poe1");

/** Folder names used by translation collections whose names differ from BCP-47 tags. */
const SOURCE_DIRS = { "zh-CN": "zh-rCN" };

const charmDir = path.resolve(argOf("--charm", process.env.POECHARM_DIR ?? path.join(repo, "../PoeCharm-main")));
const sourceDir = path.resolve(argOf("--source", path.join(charmDir, "Data/Translate", argOf("--translate-dir", SOURCE_DIRS[locale] ?? locale))));
const outDir = path.resolve(argOf("--out-dir", path.join(repo, "src/lib/i18n/data/locales", locale)));
const overridesFile = args.includes("--no-overrides")
  ? ""
  : path.resolve(argOf("--overrides", path.join(repo, "src/lib/i18n/data/overrides", `${locale}.json`)));
const bridgeFrom = argOf("--bridge-from", "");

if (!fs.existsSync(sourceDir)) {
  console.error(`No translation CSV folder at ${sourceDir}`);
  console.error("Pass --source <dir> or --charm <dir> [--translate-dir <name>].");
  process.exit(1);
}

/** Minimal RFC4180 reader: quoted fields, doubled quotes, comma separators. */
function readCsv(file) {
  const text = fs.readFileSync(file, "utf8").replace(/^\uFEFF/, "");
  const rows = [];
  let row = [];
  let field = "";
  let quoted = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (quoted) {
      if (c === '"') {
        if (text[i + 1] === '"') {
          field += '"';
          i++;
        } else quoted = false;
      } else field += c;
      continue;
    }
    if (c === '"') {
      quoted = true;
      continue;
    }
    if (c === ",") {
      row.push(field);
      field = "";
      continue;
    }
    if (c === "\n") {
      row.push(field);
      rows.push(row);
      row = [];
      field = "";
      continue;
    }
    if (c === "\r") continue;
    field += c;
  }
  if (field !== "" || row.length) {
    row.push(field);
    rows.push(row);
  }
  return rows;
}

/**
 * Whitelist of CSVs that hold interface wording rather than game objects.
 * Anything not listed stays scoped to one game. Keeping the whitelist explicit
 * means a newly added/unknown table never leaks into a different game.
 */
const UI_FILES = new Set([
  "Build.csv", "BuildDisplayStats.csv", "BuildListControl.csv",
  "CalcBreakdown.csv", "CalcDefence.csv", "CalcOffence.csv", "CalcSections.csv", "CalcSetup.csv", "CalcsTab.csv",
  "CompareBuySimilar.csv", "ComparePowerReportListControl.csv", "CompareTab.csv",
  "ConfigOptions.csv", "ConfigTab.csv", "Data.csv", "ExtBuildListControl.csv",
  "GUI.csv", "GemSelectControl.csv", "Help.csv", "ImportTab.csv",
  "ItemDBControl.csv", "ItemListControl.csv", "ItemSetListControl.csv", "ItemsTab.csv",
  "Main.csv", "MinionListControl.csv", "NotesTab.csv", "PartyTab.csv",
  "PassiveTreeView.csv", "PoBArchivesProvider.csv", "PowerReportListControl.csv",
  "SharedItemListControl.csv", "SharedItemSetListControl.csv", "SkillsTab.csv",
  "TimelessJewelListControl.csv", "TradeQuery.csv", "TradeQueryGenerator.csv",
  "Tree.csv", "TreeTab.csv", "Unsorted.csv", "Z.csv",
  "Flask_tag.csv", "Gems_tag.csv", "Minions_tag.csv",
  "stats_words_prefix.csv", "stats_words_suffix.csv",
]);

const makeTable = () => ({ exact: new Map(), patterns: new Map() });
const ui = makeTable();
const gameData = makeTable();
const stats = { files: 0, uiFiles: 0, rows: 0, conflicts: 0, dropped: 0 };

/** Merge one source -> translation pair into a table. */
function addPair(en, translation, table, overwrite = false) {
  if (!en || !translation || en === translation) return;
  stats.rows++;
  const placeholders = [...en.matchAll(/\{(\d+)\}/g)];
  if (placeholders.length === 0) {
    if (overwrite || !table.exact.has(en)) table.exact.set(en, translation);
    return;
  }
  const sequential = placeholders.every((m, i) => Number(m[1]) === i);
  const literalDigits = /\d/.test(en.replace(/\{\d+\}/g, ""));
  if (!sequential || literalDigits) {
    stats.dropped++;
    return;
  }
  if (overwrite || !table.patterns.has(en)) table.patterns.set(en, translation);
}

for (const name of fs.readdirSync(sourceDir).filter((f) => f.endsWith(".csv")).sort()) {
  stats.files++;
  const isUi = UI_FILES.has(name);
  if (isUi) stats.uiFiles++;
  const table = isUi ? ui : gameData;
  for (const row of readCsv(path.join(sourceDir, name))) {
    const en = row[0]?.trim();
    const translation = row[1]?.trim();
    // Conflicts are counted only against other CSV rows; the first table wins.
    if (en && translation && en !== translation) {
      const target = /\{\d+\}/.test(en) ? table.patterns : table.exact;
      if (target.has(en) && target.get(en) !== translation) stats.conflicts++;
    }
    addPair(en, translation, table);
  }
}

// Hand-maintained corrections win over the source tables. `_ui` entries are
// interface labels shared by all games; `_<game>` entries go to that game's
// dictionary only; everything else is game data for the current --game.
// Keys beginning with "_" inside a group are comments, not translations.
let overrides = 0;
if (overridesFile && fs.existsSync(overridesFile)) {
  const entries = Object.entries(JSON.parse(fs.readFileSync(overridesFile, "utf8")));
  for (const [en, translation] of entries) {
    if (en === "_ui" || en === `_${game}`) {
      const table = en === "_ui" ? ui : gameData;
      for (const [key, value] of Object.entries(translation)) {
        if (key.startsWith("_")) continue;
        addPair(key.trim(), String(value).trim(), table, true);
        overrides++;
      }
      continue;
    }
    if (en.startsWith("_")) continue;
    addPair(en.trim(), String(translation).trim(), gameData, true);
    overrides++;
  }
}

// Some vendored PoB data ships translated names instead of the English keys
// the CSVs are keyed by. Indexing those spellings as extra keys lets a second
// locale display its own wording. Pure data: no language is hard-coded, and a
// missing source dictionary simply produces no extra keys.
let bridged = 0;
if (bridgeFrom) {
  const bridgeDir = path.join(repo, "src/lib/i18n/data/locales", bridgeFrom);
  for (const [table, file] of [[ui, "ui.json"], [gameData, `${game}.json`]]) {
    const source = path.join(bridgeDir, file);
    if (!fs.existsSync(source)) continue;
    const other = JSON.parse(fs.readFileSync(source, "utf8"));
    for (const [en, translation] of [...table.exact]) {
      const variant = other.exact[en];
      if (!variant || variant === en || variant === translation) continue;
      if (table.exact.has(variant)) continue;
      table.exact.set(variant, translation);
      bridged++;
    }
  }
}

/** Translation sources often key UI labels with a trailing colon; PoB does not. */
function addColonAliases(table) {
  const COLON = /^(.*?)\s*[:：]\s*$/;
  for (const map of [table.exact, table.patterns]) {
    // Snapshot: aliases must not themselves be treated as sources.
    for (const [en, translation] of [...map]) {
      const base = en.match(COLON)?.[1]?.trim();
      if (!base || map.has(base)) continue;
      const baseTranslation = translation.match(COLON)?.[1]?.trim() ?? translation;
      if (baseTranslation) map.set(base, baseTranslation);
    }
  }
}
addColonAliases(ui);
addColonAliases(gameData);

const meta = {
  source: path.basename(sourceDir),
  locale,
  game,
  files: stats.files,
  overrides,
  bridged,
  ...(bridgeFrom ? { bridgeFrom } : {}),
};
function write(file, table, scope) {
  const payload = {
    meta: { ...meta, scope, exact: table.exact.size, patterns: table.patterns.size },
    exact: Object.fromEntries(table.exact),
    patterns: Object.fromEntries(table.patterns),
  };
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, JSON.stringify(payload), "utf8");
  console.log(`wrote ${file} (${(fs.statSync(file).size / 1024 / 1024).toFixed(2)} MB)  ${table.exact.size} exact / ${table.patterns.size} patterns`);
}

write(path.join(outDir, "ui.json"), ui, "ui");
write(path.join(outDir, `${game}.json`), gameData, `${game}-data`);

console.log(`locale ${locale} / game ${game}: read ${stats.rows} pairs from ${stats.files} CSVs (${stats.uiFiles} UI, ${overrides} overrides)`);
console.log(`dropped templates: ${stats.dropped}  conflicts (first wins): ${stats.conflicts}`);
