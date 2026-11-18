import { emit } from "@tauri-apps/api/event";
import { engine, type SharedItem } from "./engine.svelte";

export const SHARED_ITEMS_KEY = "pob-redux:shared-items";
export const SHARED_ITEMS_CHANGED = "shared-items:changed";

function persist(raw: string, add: boolean) {
  try {
    const raws: string[] = JSON.parse(localStorage.getItem(SHARED_ITEMS_KEY) ?? "[]");
    if (add) raws.push(raw);
    else {
      const i = raws.indexOf(raw);
      if (i >= 0) raws.splice(i, 1);
    }
    localStorage.setItem(SHARED_ITEMS_KEY, JSON.stringify(raws));
  } catch {}
}

export async function saveTemplate(itemId: number): Promise<SharedItem[] | null> {
  const r = await engine.addSharedItem({ itemId }).catch(() => null);
  if (!r) return null;
  persist(r.items[r.items.length - 1].raw, true);
  emit(SHARED_ITEMS_CHANGED).catch(() => {});
  return r.items;
}

export async function removeTemplate(item: SharedItem): Promise<SharedItem[] | null> {
  const r = await engine.removeSharedItem(item.index).catch(() => null);
  if (!r) return null;
  persist(item.raw, false);
  emit(SHARED_ITEMS_CHANGED).catch(() => {});
  return r.items;
}
