#!/usr/bin/env bun
/**
 * Build release notes for a tag from the Conventional Commits since the
 * previous tag, and print them to stdout.
 *
 *   bun scripts/release-notes.mjs            # notes for the newest tag
 *   bun scripts/release-notes.mjs v0.1.1     # notes for a specific tag
 *
 * Needs full history and tags: a shallow checkout has neither, so CI clones
 * with fetch-depth: 0.
 */
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const git = (argv) => execFileSync("git", argv, { cwd: root }).toString().trim();

const REPO = "pobredux/pob-redux";

// Order is the order sections appear. `chore` and `style` are deliberately
// absent: they say nothing to someone deciding whether to update.
const SECTIONS = [
  ["feat", "Features"],
  ["fix", "Fixes"],
  ["perf", "Performance"],
  ["revert", "Reverts"],
  ["docs", "Documentation"],
  ["refactor", "Internal"],
  ["build", "Build"],
  ["ci", "CI"],
  ["test", "Tests"],
];

const tag = process.argv[2] || git(["describe", "--tags", "--abbrev=0"]);

// The tag before this one, by version order rather than date so an out-of-order
// tag push does not pick the wrong base.
const tags = git(["tag", "--sort=-v:refname"]).split("\n").filter(Boolean);
const prev = tags[tags.indexOf(tag) + 1];

const range = prev ? `${prev}..${tag}` : tag;
// %x1f separates fields, %x1e separates records: neither appears in a message.
const raw = git(["log", range, "--no-merges", "--pretty=format:%s%x1f%H%x1e"]);

const commits = raw
  .split("\x1e")
  .map((r) => r.trim())
  .filter(Boolean)
  .map((r) => {
    const [subject, sha] = r.split("\x1f");
    const m = /^(\w+)(?:\(([^)]*)\))?(!)?:\s*(.+)$/.exec(subject);
    return m
      ? { type: m[1], scope: m[2] ?? null, breaking: Boolean(m[3]), text: m[4], sha }
      : { type: null, scope: null, breaking: false, text: subject, sha };
  })
  // Version bumps are noise in a list describing that same version.
  .filter((c) => !(c.type === "chore" && c.scope === "release"));

const out = [];

const breaking = commits.filter((c) => c.breaking);
if (breaking.length) {
  out.push("### Breaking changes", "");
  for (const c of breaking) out.push(`- ${c.scope ? `**${c.scope}:** ` : ""}${c.text}`);
  out.push("");
}

// A Path of Building pin bump is a chore that changes the numbers, so it is listed.
const pins = commits.filter((c) => c.type === "chore" && c.scope === "pob" && !c.breaking);
if (pins.length) {
  out.push("### Path of Building", "");
  for (const c of pins) out.push(`- ${c.text}`);
  out.push("");
}

for (const [type, heading] of SECTIONS) {
  const group = commits.filter((c) => c.type === type && !c.breaking);
  if (!group.length) continue;
  out.push(`### ${heading}`, "");
  for (const c of group) out.push(`- ${c.scope ? `**${c.scope}:** ` : ""}${c.text}`);
  out.push("");
}

if (!out.length) out.push("No user-facing changes.", "");

// Which upstream revision this build's calculations come from. For this app
// that is as much a part of the release as the code.
const syncToml = readFileSync(join(root, "pob-sync.toml"), "utf8");
const pinOf = (game) => new RegExp(`^\\[${game}\\][^\\[]*?^commit\\s*=\\s*"([^"]+)"`, "ms").exec(syncToml)?.[1];
const builds = [
  ["PoE2", pinOf("poe2"), "PathOfBuilding-PoE2"],
  ["PoE1", pinOf("poe1"), "PathOfBuilding"],
]
  .filter(([, pinned]) => pinned)
  .map(([game, pinned, repo]) => `[Path of Building (${game}) \`${pinned.slice(0, 10)}\`](https://github.com/PathOfBuildingCommunity/${repo}/commit/${pinned})`);
if (builds.length) out.push(`Built against ${builds.join(" and ")}.`, "");

out.push(
  "Windows: run the `.exe` installer. Linux: `.rpm` on Fedora and openSUSE, `.deb` on Debian, Ubuntu and Chromebooks, or the `.AppImage`. The `aarch64` and `arm64` files are for ARM64 Linux. macOS: open the `.dmg`.",
  "Installed copies update themselves once a newer release is published.",
  "",
  "[Code signing policy](https://pobredux.com/code-signing) · [Privacy policy](https://pobredux.com/privacy)",
  "",
);

if (prev) out.push(`**Full changelog:** https://github.com/${REPO}/compare/${prev}...${tag}`);

console.log(out.join("\n").trim());
