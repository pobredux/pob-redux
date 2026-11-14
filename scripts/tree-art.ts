// Uploading needs R2_ACCOUNT_ID, R2_ACCESS_KEY_ID, R2_SECRET_ACCESS_KEY and R2_BUCKET; checking needs nothing.
import { S3Client } from "bun";

const BASE = "https://art.pobredux.com/tree";
const STORE = "target/tree-art/store";
const mode = process.argv[2];
if (mode !== "check" && mode !== "upload") {
  console.error("usage: bun scripts/tree-art.ts check | upload");
  process.exit(2);
}

const keys = new Map<string, string>();
for (const game of ["poe1", "poe2"]) {
  const file = Bun.file(`trees/${game}/art.json`);
  if (!(await file.exists())) continue;
  const art = (await file.json()) as { files: Record<string, { hash: string }> };
  for (const [rel, { hash }] of Object.entries(art.files)) {
    const ext = rel.slice(rel.lastIndexOf(".") + 1);
    keys.set(`${hash}.${ext}`, ext === "png" ? "image/png" : "image/webp");
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
  await pool([...keys.keys()], async (key) => {
    const res = await fetch(`${BASE}/${key}`, { method: "HEAD" });
    if (!res.ok) missing.push(key);
  });
  console.log(`${keys.size - missing.length} of ${keys.size} tree art files are on R2`);
  if (missing.length) {
    console.error(`missing: ${missing.slice(0, 10).join(", ")}${missing.length > 10 ? ", ..." : ""}`);
    console.error("run `bun scripts/tree-art.ts upload` after `bun run sync`");
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
  let continuationToken: string | undefined;
  do {
    const page = await s3.list({ prefix: "tree/", continuationToken });
    for (const item of page.contents ?? []) existing.add(item.key);
    continuationToken = page.isTruncated ? page.nextContinuationToken : undefined;
  } while (continuationToken);
  const todo = [...keys].filter(([key]) => !existing.has(`tree/${key}`));
  let bytes = 0;
  await pool(todo, async ([key, type]) => {
    const file = Bun.file(`${STORE}/${key}`);
    if (!(await file.exists())) throw new Error(`${STORE}/${key} is missing; run \`bun run sync\` first`);
    bytes += file.size;
    await s3.write(`tree/${key}`, file, { type });
  });
  console.log(`uploaded ${todo.length} files (${(bytes / 1048576).toFixed(1)} MB), ${keys.size - todo.length} already on R2`);
}
