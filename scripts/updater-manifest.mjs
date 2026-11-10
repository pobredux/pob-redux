import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const BUNDLES = [
  { suffix: "_x64-setup.exe", keys: ["windows-x86_64", "windows-x86_64-nsis"] },
  { suffix: "_aarch64.app.tar.gz", keys: ["darwin-aarch64", "darwin-aarch64-app"] },
  { suffix: "_amd64.AppImage", keys: ["linux-x86_64", "linux-x86_64-appimage"] },
  { suffix: "_amd64.deb", keys: ["linux-x86_64-deb"] },
  { suffix: ".x86_64.rpm", keys: ["linux-x86_64-rpm"] },
  { suffix: "_aarch64.AppImage", keys: ["linux-aarch64", "linux-aarch64-appimage"] },
  { suffix: "_arm64.deb", keys: ["linux-aarch64-deb"] },
  { suffix: ".aarch64.rpm", keys: ["linux-aarch64-rpm"] },
];

const args = process.argv.slice(2);
const dryRun = args.includes("--dry-run");
const [tag, repo] = args.filter((a) => !a.startsWith("--"));
if (!tag || !repo) {
  console.error("usage: updater-manifest.mjs <tag> <owner/repo> [--dry-run]");
  process.exit(2);
}

const gh = (...a) => execFileSync("gh", a, { encoding: "utf8" });

const { assets } = JSON.parse(gh("release", "view", tag, "-R", repo, "--json", "assets"));
const dir = mkdtempSync(join(tmpdir(), "updater-manifest-"));
gh("release", "download", tag, "-R", repo, "-p", "*.sig", "-p", "latest.json", "-D", dir);

const platforms = {};
const missing = [];
for (const { suffix, keys } of BUNDLES) {
  const bundle = assets.find((a) => a.name.endsWith(suffix));
  const sig = bundle && assets.find((a) => a.name === `${bundle.name}.sig`);
  if (!sig) {
    missing.push(...keys);
    continue;
  }
  const entry = { signature: readFileSync(join(dir, sig.name), "utf8"), url: bundle.apiUrl };
  for (const key of keys) platforms[key] = entry;
}
if (missing.length) {
  console.error(`::error::no signed bundle on ${tag} for: ${missing.join(" ")}`);
  process.exit(1);
}

const path = join(dir, "latest.json");
const manifest = { ...JSON.parse(readFileSync(path, "utf8")), platforms };
writeFileSync(path, JSON.stringify(manifest, null, 2));

if (dryRun) {
  console.log(JSON.stringify(manifest, null, 2));
} else {
  gh("release", "upload", tag, path, "-R", repo, "--clobber");
  console.log(Object.keys(platforms).join(" "));
}
