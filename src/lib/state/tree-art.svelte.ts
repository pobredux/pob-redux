import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { game } from "$lib/state/game.svelte";

export interface TreeArtStatus {
  game: "poe1" | "poe2";
  state: "none" | "checking" | "downloading" | "ready" | "failed";
  done: number;
  total: number;
  error: string | null;
}

/** The passive tree art download the app runs on first launch and after updates. */
class TreeArtStore {
  private byGame = $state<Partial<Record<"poe1" | "poe2", TreeArtStatus>>>({});
  /** Bumps when art becomes ready after a failure, so a tree loaded without it reloads. */
  generation = $state(0);
  private started: Promise<void> | null = null;
  private waiters: (() => void)[] = [];
  private finished: Partial<Record<"poe1" | "poe2", TreeArtStatus["state"]>> = {};

  status = $derived(this.byGame[game.current] ?? null);
  busy = $derived(this.status?.state === "checking" || this.status?.state === "downloading");

  private init(): Promise<void> {
    this.started ??= (async () => {
      await listen<TreeArtStatus>("tree-art", (e) => this.set(e.payload));
      this.set(await invoke<TreeArtStatus>("tree_art_status"));
    })();
    return this.started;
  }

  private set(s: TreeArtStatus) {
    this.byGame[s.game] = s;
    if (s.state === "checking" || s.state === "downloading") return;
    if (s.state === "ready" && this.finished[s.game] === "failed") this.generation++;
    this.finished[s.game] = s.state;
    if (s.game === game.current) {
      for (const resolve of this.waiters.splice(0)) resolve();
    }
  }

  /** Resolves once the current game's art is on disk, has failed, or does not exist. */
  async settled(): Promise<void> {
    await this.init();
    if (!this.byGame[game.current]) this.set(await invoke<TreeArtStatus>("tree_art_status"));
    if (!this.busy) return;
    return new Promise((resolve) => this.waiters.push(resolve));
  }

  retry() {
    void invoke("tree_art_retry");
  }
}

export const treeArt = new TreeArtStore();
