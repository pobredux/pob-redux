// Uploading needs R2_ACCOUNT_ID, R2_ACCESS_KEY_ID, R2_SECRET_ACCESS_KEY and R2_BUCKET; checking needs nothing.
import { S3Client } from "bun";

const BASE = "https://art.pobredux.com";
const mode = process.argv[2];
if (mode !== "check" && mode !== "upload") {
  console.error("usage: bun scripts/downloads.ts check | upload");
  process.exit(2);
}

/** R2 key -> where the file is on disk here, and its type. */
const files = new Map<string, { source: string; type: string }>();
for (const game of ["poe1", "poe2"]) {
  const art = Bun.file(`trees/${game}/art.json`);
  if (!(await art.exists())) continue;
  for (const [rel, { hash }] of Object.entries(((await art.json()) as { files: Record<string, { hash: string }> }).files)) {
    const ext = rel.slice(rel.lastIndexOf(".") + 1);
    files.set(`tree/${hash}.${ext}`, { source: `target/tree-art/store/${hash}.${ext}`, type: ext === "png" ? "image/png" : "image/webp" });
  }
}
for (const root of ["pob", "pob1"]) {
  const list = Bun.file(`src-tauri/resources/${root}/downloads.json`);
  if (!(await list.exists())) continue;
  for (const [rel, { hash }] of Object.entries(((await list.json()) as { files: Record<string, { hash: string }> }).files)) {
    files.set(`data/${hash}`, { source: `src-tauri/resources/${root}-data/${rel}`, type: "application/octet-stream" });
  }
}

async function pool<T>(items: T[], work: (item: T) => Promise<void>, size = 16) {
  let next = 0;
  await Promise.all(Array.from({ length: size }, async () => {
    while (next < items.length) await work(items[next++]!);
  }));
}

if (mode === "check") {
  const missing: string[] = [];
  await pool([...files.keys()], async (key) => {
    const res = await fetch(`${BASE}/${key}`, { method: "HEAD" });
    if (!res.ok) missing.push(key);
  });
  console.log(`${files.size - missing.length} of ${files.size} downloadable files are on R2`);
  if (missing.length) {
    console.error(`missing: ${missing.slice(0, 10).join(", ")}${missing.length > 10 ? ", ..." : ""}`);
    console.error("run `bun scripts/downloads.ts upload` after `bun run sync`");
    process.exit(1);
  }
} else {
  const env = (name: string) => process.env[name] || (() => { throw new Error(`${name} is not set`); })();
  const s3 = new S3Client({
    accessKeyId: env("R2_ACCESS_KEY_ID"),
    secretAccessKey: env("R2_SECRET_ACCESS_KEY"),
    bucket: env("R2_BUCKET"),
    endpoint: `https://${env("R2_ACCOUNT_ID")}.r2.cloudflarestorage.com`,
  });
  const existing = new Set<string>();
  for (const prefix of ["tree/", "data/"]) {
    let continuationToken: string | undefined;
    do {
      const page = await s3.list({ prefix, continuationToken });
      for (const item of page.contents ?? []) existing.add(item.key);
      continuationToken = page.isTruncated ? page.nextContinuationToken : undefined;
    } while (continuationToken);
  }
  const todo = [...files].filter(([key]) => !existing.has(key));
  let bytes = 0;
  await pool(todo, async ([key, { source, type }]) => {
    const file = Bun.file(source);
    if (!(await file.exists())) throw new Error(`${source} is missing; run \`bun run sync\` first`);
    bytes += file.size;
    await s3.write(key, file, { type });
  });
  console.log(`uploaded ${todo.length} files (${(bytes / 1048576).toFixed(1)} MB), ${files.size - todo.length} already on R2`);
}
