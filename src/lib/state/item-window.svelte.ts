import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { build } from "$lib/state/build.svelte";

class ItemWindowStore {
  open = $state(false);
  itemId = $state<number | null>(null);
  /** Set when the window docks its item back; the Items view opens it in place and clears this. */
  docked = $state<number | null>(null);
  private started = false;

  async show(itemId: number) {
    this.itemId = itemId;
    this.open = true;
    await invoke("item_window_open", { itemId }).catch(() => {
      this.open = false;
    });
  }

  close() {
    void invoke("item_window_close").catch(() => {});
  }

  init() {
    if (this.started) return;
    this.started = true;
    void listen("item-window:closed", () => {
      this.open = false;
      this.itemId = null;
    });
    void listen<number>("item-window:dock", (event) => {
      this.open = false;
      this.itemId = null;
      this.docked = event.payload;
    });
    void listen("item:edited", () => {
      build.userEdits++;
      void build.sync().catch(() => {});
    });
    $effect.root(() => {
      let generation: number | undefined;
      $effect(() => {
        const current = build.info?.generation;
        if (generation !== undefined && current !== generation && this.open) this.close();
        generation = current;
      });
    });
  }
}

export const itemWindow = new ItemWindowStore();
