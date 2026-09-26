import { engine, status as engineStatus, appPaths, sessionInfo, writeTextFile, type EngineStatus, type AppPaths } from "$lib/engine.svelte";
import { build, autosaveKey } from "$lib/state/build.svelte";
import { confirm } from "$lib/state/confirm.svelte";
import { appOptions } from "$lib/state/options.svelte";
import { mcp } from "$lib/state/mcp.svelte";
import { chat } from "$lib/state/chat.svelte";
import { appUpdate } from "$lib/state/update.svelte";
import { game } from "$lib/state/game.svelte";
import { links } from "$lib/state/links.svelte";
import { m } from "$lib/paraglide/messages";

/**
 * The engine's boot state and the app's boot sequence. `boot()` runs once at
 * start and again after every game switch, since a switch replaces the
 * engine: it waits for the engine, then brings every store in line with it.
 */
class AppStore {
  status = $state<EngineStatus | null>(null);
  paths = $state<AppPaths | null>(null);
  private timer = 0;
  private booted = false;

  async boot() {
    clearTimeout(this.timer);
    this.status = null;
    await game.init().catch(() => {});
    const st = await new Promise<EngineStatus>((resolve) => {
      const poll = async () => {
        let s: EngineStatus;
        try {
          s = await engineStatus();
        } catch (e) {
          s = { state: "error", message: String(e), boot_ms: null, pob_root: "", user_dir: "" };
        }
        this.status = s;
        if (s.state === "booting") this.timer = window.setTimeout(poll, 150);
        else resolve(s);
      };
      poll();
    });
    if (st.state !== "ready") return;

    const first = !this.booted;
    this.booted = true;
    this.paths = await appPaths().catch(() => null);
    await appOptions.init().catch(() => {});
    // The MCP server and the assistant are PoE2 features.
    if (game.isPoe2) {
      await mcp.init().catch(() => {});
      await chat.init(this.paths?.chat_open).catch(() => {});
      if (first) {
        if (this.paths?.chat_provider) await chat.setProvider(this.paths.chat_provider).catch(() => {});
        if (this.paths?.chat_model) chat.setModel(this.paths.chat_model);
      }
    } else {
      chat.open = false;
      await mcp.refresh().catch(() => {});
    }
    if (first) appUpdate.init();
    // shared items added in this app are ours to restore (PoB's own
    // settings file, which also holds shared items, is never written)
    try {
      const raws: string[] = JSON.parse(localStorage.getItem("pob-redux:shared-items") ?? "[]");
      for (const raw of raws) await engine.addSharedItem({ raw }).catch(() => {});
    } catch {}
    const session = first ? await sessionInfo().catch(() => null) : null;
    const linked = first ? await links.init().catch(() => false) : false;
    if (linked) {
      // the build from the link the app was opened with is loaded
    } else if (first && this.paths?.open_on_start) {
      await build.loadFile(this.paths.open_on_start);
    } else if (session?.safeMode) {
      await build.run(async () => {}, { sync: true });
      build.say(m.app_safe_mode());
    } else if (session?.uncleanExit && (await this.declineRecovery())) {
      await build.run(async () => {}, { sync: true });
    } else if (!(await build.reopenLast())) {
      await build.run(async () => {}, { sync: true });
    }
    build.view = ((first && this.paths?.initial_view) as typeof build.view) || "tree";
    if (first && game.isPoe2) {
      if (this.paths?.chat_allow) chat.allowWrites = true;
      if (this.paths?.chat_log) chat.logPath = this.paths.chat_log;
      if (this.paths?.chat_ask) {
        chat.input = this.paths.chat_ask;
        // A value starting with "/" only fills the box, so the tool menu can
        // be inspected without spending a request.
        if (!this.paths.chat_ask.startsWith("/")) void chat.send();
      }
    }
  }

  /** True when the user chose Start empty, after a recovered copy is saved. */
  private async declineRecovery(): Promise<boolean> {
    let saved: { name?: string; xml?: string } | null = null;
    try {
      saved = JSON.parse(localStorage.getItem(autosaveKey()) ?? "null");
    } catch {}
    if (!saved?.xml) return false;
    const name = saved.name || "build";
    const reopen = await confirm.ask({
      title: m.app_recover_title(),
      message: m.app_recover_message({ build: saved.name ? `"${saved.name}"` : m.app_recover_build_fallback() }),
      ok: m.app_recover_ok(),
      cancel: m.app_recover_cancel(),
    });
    if (reopen) return false;
    const dir = this.paths?.builds_dir;
    if (dir) {
      const d = new Date();
      const two = (n: number) => String(n).padStart(2, "0");
      const stamp = `${d.getFullYear()}-${two(d.getMonth() + 1)}-${two(d.getDate())} ${two(d.getHours())}-${two(d.getMinutes())}`;
      const file = `${m.app_recovered_prefix({ name: name.replace(/[\\/:*?"<>|]/g, ""), stamp })}.xml`;
      await writeTextFile(`${dir}/${file}`, saved.xml)
        .then(() => build.say(m.app_recovered_saved({ file })))
        .catch((e) => (build.error = m.app_recovered_failed({ error: String(e) })));
    }
    return true;
  }
}

export const app = new AppStore();
