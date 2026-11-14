import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { build } from "$lib/state/build.svelte";
import { game } from "$lib/state/game.svelte";
import type { TreeArtStatus } from "$lib/state/tree-art.svelte";

/** PoE1's timeless jewel archives, which the app downloads after the tree art. */
class GameDataStore {
  private byGame = $state<Partial<Record<"poe1" | "poe2", TreeArtStatus>>>({});
  private started = false;
  /** True for a few seconds after a download that fetched files finishes. */
  justDone = $state(false);
  private doneTimer = 0;

  status = $derived(this.byGame[game.current] ?? null);

  async init() {
    if (this.started) return;
    this.started = true;
    await listen<TreeArtStatus>("game-data", (e) => {
      const s = e.payload;
      const before = this.byGame[s.game]?.state;
      this.byGame[s.game] = s;
      if (s.state !== "ready" || before !== "downloading" || s.total === 0 || s.game !== game.current) return;
      // The engine has re-read the archives; the open build's numbers changed with them.
      if (build.info) void build.sync();
      this.justDone = true;
      clearTimeout(this.doneTimer);
      this.doneTimer = window.setTimeout(() => (this.justDone = false), 4000);
    });
    const s = await invoke<TreeArtStatus>("game_data_status");
    this.byGame[s.game] ??= s;
  }
}

export const gameData = new GameDataStore();
