import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "$lib/engine.svelte";

const KEY = "pob-redux:trade-window";

class TradeWindow {
  inApp = $state(false);

  constructor() {
    try {
      this.inApp = localStorage.getItem(KEY) === "1";
    } catch {}
  }

  setInApp(on: boolean) {
    this.inApp = on;
    try {
      localStorage.setItem(KEY, on ? "1" : "0");
    } catch {}
  }

  /** Opens a trade search in the app's trade window, or the browser when that is off or fails. */
  async open(url: string) {
    if (this.inApp) {
      try {
        await invoke("trade_window_open", { url });
        return;
      } catch (e) {
        console.warn("trade window:", e);
      }
    }
    await openUrl(url);
  }
}

export const tradeWindow = new TradeWindow();
