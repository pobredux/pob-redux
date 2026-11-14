// usage: bun scripts/cursors.ts <the game's exported art/2dart/cursors folder>
import { readdirSync, mkdirSync, rmSync } from "node:fs";
import { join } from "node:path";

const src = process.argv[2];
if (!src) {
  console.error("usage: bun scripts/cursors.ts <art/2dart/cursors folder>");
  process.exit(2);
}
const out = "src/lib/assets/cursors";
const STATES = ["", "hover", "click"] as const;
const SIZE = 32;

function ffmpeg(args: string[]): Buffer {
  const r = Bun.spawnSync(["ffmpeg", "-loglevel", "error", "-y", ...args], { stdout: "pipe", stderr: "pipe" });
  if (r.exitCode !== 0) throw new Error(`ffmpeg ${args.join(" ")}: ${r.stderr.toString()}`);
  return r.stdout;
}

/** The arrow's tip: the opaque pixel nearest the top-left corner. */
function tip(file: string): [number, number] {
  const probe = Bun.spawnSync(["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height", "-of", "csv=p=0", file]);
  const [w, h] = probe.stdout.toString().trim().split(",").map(Number);
  const rgba = ffmpeg(["-i", file, "-f", "rawvideo", "-pix_fmt", "rgba", "-"]);
  let best: [number, number] = [0, 0];
  let bestSum = Infinity;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if (rgba[(y * w + x) * 4 + 3] > 128 && x + y < bestSum) {
        bestSum = x + y;
        best = [x / w, y / h];
      }
    }
  }
  return best;
}

const files = new Set(readdirSync(src));
const skins = [...files]
  .filter((f) => f.endsWith(".bmp") && !/_(hover|click)\.bmp$|_\d+x\d+|^cursornext/.test(f))
  .map((f) => f.slice(0, -4))
  .filter((skin) => STATES.every((s) => files.has(`${skin}${s ? `_${s}` : ""}.bmp`)))
  .sort((a, b) => (a === "default" ? -1 : b === "default" ? 1 : a.localeCompare(b)));

rmSync(out, { recursive: true, force: true });
const manifest: { id: string; hotspot: [number, number] }[] = [];
for (const skin of skins) {
  mkdirSync(join(out, skin), { recursive: true });
  for (const state of STATES) {
    const file = join(src, `${skin}${state ? `_${state}` : ""}.bmp`);
    const name = state || "arrow";
    for (const [scale, suffix] of [[1, ""], [2, "@2x"]] as const) {
      const px = SIZE * scale;
      ffmpeg(["-i", file, "-vf", `scale=${px}:${px}:flags=lanczos,format=rgba`, join(out, skin, `${name}${suffix}.png`)]);
    }
  }
  const [tx, ty] = tip(join(src, `${skin}.bmp`));
  manifest.push({ id: skin, hotspot: [Math.floor(tx * SIZE), Math.floor(ty * SIZE)] });
}
await Bun.write(join(out, "cursors.json"), `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`${skins.length} cursors: ${skins.join(", ")}`);
