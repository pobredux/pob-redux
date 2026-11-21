import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { build } from "$lib/state/build.svelte";
import { app } from "$lib/state/app.svelte";
import { confirm } from "$lib/state/confirm.svelte";
import { m } from "$lib/paraglide/messages";

export type Game = "poe1" | "poe2";

export interface GameStatus {
  game: Game;
  firstRun: boolean;
}

export const GAMES: Game[] = ["poe1", "poe2"];
export const GAME_LABEL: Record<Game, string> = { poe1: "Path of Exile 1", poe2: "Path of Exile 2" };
export const GAME_SHORT: Record<Game, string> = { poe1: "POE1", poe2: "POE2" };

/**
 * Which game the session is on. Each game runs its own Path of Building in
 * the engine, so a change reboots it and the app re-runs its boot sequence.
 * The choice is saved by the host; the first run asks.
 */
class GameStore {
  current = $state<Game>("poe2");
  firstRun = $state(false);
  switching = $state(false);
  error = $state<string | null>(null);

  get isPoe2() {
    return this.current === "poe2";
  }
  get isPoe1() {
    return this.current === "poe1";
  }

  async init() {
    this.apply(await invoke<GameStatus>("game_status"));
  }

  private apply(s: GameStatus) {
    this.current = s.game;
    this.firstRun = s.firstRun;
  }

  /** Choose a game. A different game reboots the engine. Returns false if the user kept the current one. */
  async choose(g: Game): Promise<boolean> {
    if (g === this.current) {
      if (this.firstRun) this.apply(await invoke<GameStatus>("set_game", { game: g }));
      return true;
    }
    // On the first run nothing the user did is at stake: the app booted PoE2
    // and reopened its last snapshot on its own.
    if (build.info?.unsaved && !this.firstRun) {
      const ok = await confirm.ask({
        title: m.game_switch_title(),
        message: m.game_switch_message({ game: GAME_LABEL[g] }),
        ok: m.game_switch_ok(),
        cancel: m.game_switch_cancel(),
      });
      if (!ok) return false;
    }
    this.switching = true;
    this.error = null;
    try {
      this.apply(await invoke<GameStatus>("set_game", { game: g }));
      build.reset();
      await app.boot();
      // Torn-off tabs reload for the other game once its engine is up.
      void emit("game:changed").catch(() => {});
      return true;
    } catch (e) {
      this.error = String(e);
      return false;
    } finally {
      this.switching = false;
    }
  }
}

export const game = new GameStore();
