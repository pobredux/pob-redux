// Applies PoeCharm's Simplified Chinese name tables to the vendored PoE1 data.
//
//   node scripts/apply-poe1-cn-names.mjs [--charm <PoeCharm dir>] [--pob <pob1 dir>]
//
// Only display fields are touched: the gem `name` in Data/Gems.lua and the node
// `name` in TreeData/<version>/tree.json. Mod text, base types and everything
// the calculator matches on stay English, so numbers and item parsing are
// unchanged. Each rewritten gem keeps its English name in `englishName`, which
// the engine bridge uses to rebuild PoB's English name index.
//
// Re-run after `bun run sync`, which restores the upstream files.

import fs from "node:fs";
import path from "node:path";

const args = process.argv.slice(2);
const argOf = (flag, fallback) => {
  const i = args.indexOf(flag);
  return i >= 0 && args[i + 1] ? args[i + 1] : fallback;
};

const repo = path.resolve(import.meta.dirname, "..");
const charmDir = path.resolve(
  argOf("--charm", "C:\\Users\\20158\\Desktop\\PoeCharm-main"),
);
const pobDir = path.resolve(argOf("--pob", path.join(repo, "src-tauri/resources/pob1")));
const zhDir = path.join(charmDir, "Data/Translate/zh-rCN");

if (!fs.existsSync(zhDir)) {
  console.error(`No zh-rCN translation folder at ${zhDir}`);
  process.exit(1);
}
if (!fs.existsSync(path.join(pobDir, "Data/Gems.lua"))) {
  console.error(`No PoB data at ${pobDir}`);
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

/** English -> Chinese pairs from one table, trimmed, blanks dropped. */
function readPairs(file) {
  const map = new Map();
  if (!fs.existsSync(file)) return map;
  for (const [en, zh] of readCsv(file)) {
    const k = en?.trim();
    const v = zh?.trim();
    if (k && v && k !== v) map.set(k, v);
  }
  return map;
}

const report = { gems: 0, gemMisses: [], nodes: 0, trees: 0, nodeMisses: new Set() };

// ---------------------------------------------------------------- gem names
// Active skills come from Gems_data.txt.csv (keyed by skill name); support gems
// are missing there and come from Items_Gems.txt.csv, which marks them 「（辅）」.
const gemData = readPairs(path.join(zhDir, "Gems_data.txt.csv"));
const gemItems = readPairs(path.join(zhDir, "Items_Gems.txt.csv"));

const gemName = (en) => {
  const fromData = gemData.get(en);
  // The skills table has a couple of rows where a Vaal name sits on the base gem.
  const dataOk = fromData && !(fromData.startsWith("瓦尔") && !/^Vaal /i.test(en));
  if (dataOk) return fromData;
  return gemItems.get(en) ?? fromData;
};

const gemsFile = path.join(pobDir, "Data/Gems.lua");
const gemsLua = fs.readFileSync(gemsFile, "utf8");
const renamed = gemsLua.replace(
  /^(\t\t)name = "([^"]+)",\r?$/gm,
  (line, indent, en) => {
    if (/[\u4e00-\u9fff]/.test(en)) return line;
    const zh = gemName(en);
    if (!zh || zh === en) {
      if (!zh) report.gemMisses.push(en);
      return line;
    }
    report.gems++;
    return `${indent}name = "${zh}",\n${indent}englishName = "${en}",`;
  },
);

// PoB runs every gem name through sanitiseText(), which replaces each non-ASCII
// byte with "?" for its bitmap font. Keep the Chinese name in `cnName` so the
// engine bridge can restore it after that pass.
// The upstream file mixes CRLF and LF, so normalise while inserting.
const lines = renamed.split(/\r?\n/);
const withCn = [];
for (let i = 0; i < lines.length; i++) {
  withCn.push(lines[i]);
  const en = /^\t\tenglishName = "([^"]+)",$/.exec(lines[i]);
  if (!en) continue;
  const nameLine = /^\t\tname = "([^"]+)",$/.exec(lines[i - 1] ?? "");
  if (!nameLine) continue;
  if ((lines[i + 1] ?? "").startsWith("\t\tcnName = ")) continue;
  withCn.push(`\t\tcnName = "${nameLine[1]}",`);
}
fs.writeFileSync(gemsFile, withCn.join("\n"), "utf8");

// ---------------------------------------------------------------- tree nodes
// tree_dn.csv is a mixed display-name table (stats, class names, node names);
// only names that actually exist as node names are used here.
const treeDn = readPairs(path.join(zhDir, "tree_dn.csv"));

const treeRoot = path.join(pobDir, "TreeData");
for (const version of fs.readdirSync(treeRoot)) {
  const file = path.join(treeRoot, version, "tree.json");
  if (!fs.existsSync(file)) continue;
  const raw = fs.readFileSync(file, "utf8");
  let tree;
  try {
    tree = JSON.parse(raw);
  } catch {
    continue;
  }
  let changed = 0;
  for (const node of Object.values(tree.nodes ?? {})) {
    if (!node?.name) continue;
    if (/[\u4e00-\u9fff]/.test(node.name)) continue;
    const zh = treeDn.get(node.name.trim());
    if (!zh || zh === node.name) {
      if (!zh && node.name.trim()) report.nodeMisses.add(node.name.trim());
      continue;
    }
    node.name = zh;
    changed++;
  }
  if (changed) {
    fs.writeFileSync(file, JSON.stringify(tree), "utf8");
    report.nodes += changed;
    report.trees++;
  }
}

// -------------------------------------------------------------- config labels
// The Configuration tab's option and section names live in Modules/ConfigOptions.
// Only `label` and `section` values are touched: `var` and list `val` are the
// keys a build file stores, so they stay English. The table's keys carry no
// colour codes while the module's labels do, hence the strip-and-retry match.
const cfgOptions = readPairs(path.join(zhDir, "ConfigOptions.csv"));
const plain = (s) => s.replace(/\^x[0-9A-Fa-f]{6}|\^[0-9]/g, "");

const cfgFile = path.join(pobDir, "Modules/ConfigOptions.lua");
let cfg = fs.readFileSync(cfgFile, "utf8");
let cfgHits = 0;
const cfgMisses = new Set();
cfg = cfg.replace(/(\b(?:label|section)\s*=\s*)"([^"]*)"/g, (whole, prefix, en) => {
  if (/[\u4e00-\u9fff]/.test(en)) return whole;
  const zh = cfgOptions.get(en) ?? cfgOptions.get(plain(en).trim());
  if (!zh) {
    if (en.trim()) cfgMisses.add(en);
    return whole;
  }
  cfgHits++;
  return `${prefix}"${zh}"`;
});
fs.writeFileSync(cfgFile, cfg, "utf8");

console.log(`gems renamed: ${report.gems}`);
if (report.gemMisses.length) {
  console.log(`gem names without a translation (${report.gemMisses.length}): ${report.gemMisses.join(", ")}`);
}
console.log(`tree nodes renamed: ${report.nodes} across ${report.trees} tree files`);
console.log(`node names without a translation: ${report.nodeMisses.size}`);
if (report.nodeMisses.size) {
  console.log([...report.nodeMisses].slice(0, 20).join(", "));
}
console.log(`config labels renamed: ${cfgHits}, without a translation: ${cfgMisses.size}`);

