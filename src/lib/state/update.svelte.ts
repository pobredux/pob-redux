import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "$lib/engine.svelte";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

const KEY = "pob-redux:update";
const DOWNLOAD_URL = "https://pobredux.com/#download";

export type UpdatePhase = "idle" | "checking" | "available" | "downloading" | "ready" | "error";
/** `self`: the app installs the update. `aur`: pacman does. `package`: a .deb or .rpm the user downloads. */
export type UpdateMethod = "self" | "aur" | "package";

/**
 * App auto-update. Checks once shortly after boot, then only when asked.
 *
 * The download is verified against the public key baked into the build before
 * anything is installed, so a compromised update host cannot ship a payload.
 */
class UpdateStore {
  phase = $state<UpdatePhase>("idle");
  version = $state<string | null>(null);
  notes = $state<string | null>(null);
  error = $state<string | null>(null);
  /** 0-100 while downloading, null when the server sends no length. */
  progress = $state<number | null>(null);
  /** Set when the user closes the banner, so it stays closed for that version. */
  dismissed = $state<string | null>(null);
  /** Linux package installs cannot self-update: the updater only replaces an AppImage. */
  method = $state<UpdateMethod>("self");

  private update: Update | null = null;
  private downloaded = 0;
  private total = 0;

  get showBanner() {
    return (
      (this.phase === "available" || this.phase === "downloading" || this.phase === "ready") &&
      this.version !== this.dismissed
    );
  }

  init() {
    try {
      this.dismissed = localStorage.getItem(KEY);
    } catch {}
    invoke<UpdateMethod>("update_method")
      .then((v) => (this.method = v))
      .catch(() => {});
    // Let the engine finish booting before touching the network.
    setTimeout(() => void this.check(false), 4000);
  }

  dismiss() {
    this.dismissed = this.version;
    try {
      if (this.version) localStorage.setItem(KEY, this.version);
    } catch {}
  }

  openReleases() {
    return openUrl(DOWNLOAD_URL);
  }

  /** `manual` surfaces "you are up to date" and any error; the boot check is silent. */
  async check(manual = true) {
    if (this.phase === "downloading" || this.phase === "checking") return;
    this.phase = "checking";
    this.error = null;
    try {
      const found = await check();
      if (!found) {
        this.phase = "idle";
        this.version = null;
        return;
      }
      this.update = found;
      this.version = found.version;
      this.notes = found.body ?? null;
      this.phase = "available";
    } catch (e) {
      this.phase = manual ? "error" : "idle";
      if (manual) this.error = String(e);
      else console.warn("update check failed:", e);
    }
  }

  async install() {
    if (!this.update) return;
    this.phase = "downloading";
    this.progress = null;
    this.downloaded = 0;
    this.total = 0;
    try {
      await this.update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          this.total = event.data.contentLength ?? 0;
        } else if (event.event === "Progress") {
          this.downloaded += event.data.chunkLength;
          this.progress = this.total ? Math.round((this.downloaded / this.total) * 100) : null;
        } else if (event.event === "Finished") {
          this.progress = 100;
        }
      });
      this.phase = "ready";
    } catch (e) {
      this.phase = "error";
      this.error = String(e);
    }
  }

  /** Windows' NSIS installer closes the app itself; elsewhere we relaunch. */
  async restart() {
    await relaunch();
  }
}

export const appUpdate = new UpdateStore();
