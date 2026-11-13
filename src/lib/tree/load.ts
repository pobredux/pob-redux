import { readTreeJson } from "$lib/engine.svelte";
import { build } from "$lib/state/build.svelte";
import { AssetStore } from "./assets";
import { parseTree, type TreeModel } from "./model";

export interface LoadedTree {
  model: TreeModel;
  assets: AssetStore | null;
}

// The tree view unmounts on every tab switch; the last tree outlives it.
let last: { version: string; tree: Promise<LoadedTree> } | null = null;

export function loadTree(version: string): Promise<LoadedTree> {
  if (last?.version !== version) {
    const tree = Promise.all([readTreeJson(version), loadArt(version)]).then(([json, assets]) => ({ model: parseTree(version, json), assets }));
    const entry = { version, tree };
    tree.catch(() => {
      if (last === entry) last = null;
    });
    last = entry;
  }
  return last.tree;
}

// pob-sync builds sprite sheets for the newest tree only.
async function loadArt(version: string): Promise<AssetStore | null> {
  const latest = build.meta?.latestTreeVersion;
  return (await AssetStore.load(version)) ?? (latest && latest !== version ? AssetStore.load(latest) : null);
}
