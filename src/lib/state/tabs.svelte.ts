import { invoke } from "@tauri-apps/api/core";
import { emitTo, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { IconName } from "$lib/components/Icon.svelte";
import { build, type ViewId } from "$lib/state/build.svelte";
import { appOptions } from "$lib/state/options.svelte";
import { ui, type Jump } from "$lib/state/ui.svelte";
import { IS_MAIN_WINDOW, TAB_VIEW, WINDOW_LABEL } from "$lib/window";
import { m } from "$lib/paraglide/messages";

const TORN_KEY = "pob-redux:torn-tabs";
const boundsKey = (view: ViewId) => `pob-redux:tab-window:${view}`;

export type Tab = { id: ViewId; label: string; key: string; icon: IconName };

/** Call inside `$derived`, so the labels follow a language change. */
export function tabList(): Tab[] {
  return [
    { id: "import", label: m.view_builds(), key: "1", icon: "folder" },
    { id: "overview", label: m.view_overview(), key: "", icon: "squares-four" },
    { id: "tree", label: m.view_tree(), key: "2", icon: "graph" },
    { id: "skills", label: m.view_skills(), key: "3", icon: "diamonds-four" },
    { id: "items", label: m.view_items(), key: "4", icon: "sword" },
    { id: "calcs", label: m.view_calcs(), key: "5", icon: "calculator" },
    { id: "config", label: m.view_config(), key: "6", icon: "sliders-horizontal" },
    { id: "notes", label: m.view_notes(), key: "7", icon: "note-pencil" },
    { id: "party", label: m.view_party(), key: "8", icon: "users-three" },
    { id: "optimise", label: m.view_optimise(), key: "9", icon: "chart-line-up" },
    { id: "compare", label: m.view_compare(), key: "", icon: "scales" },
  ];
}

export function tearable(view: ViewId) {
  return view !== "import";
}

type Bounds = { x: number; y: number; width: number; height: number };

function savedBounds(view: ViewId): Bounds | null {
  try {
    const b = JSON.parse(localStorage.getItem(boundsKey(view)) ?? "null");
    return b && [b.x, b.y, b.width, b.height].every(Number.isFinite) ? b : null;
  } catch {
    return null;
  }
}

/** Torn-off windows keep their own place and size, so the next tear-off or start puts them back there. */
export async function rememberBounds() {
  if (!TAB_VIEW) return;
  const win = getCurrentWindow();
  if (await win.isMaximized().catch(() => true)) return;
  try {
    const f = await win.scaleFactor();
    const pos = (await win.outerPosition()).toLogical(f);
    const size = (await win.innerSize()).toLogical(f);
    localStorage.setItem(boundsKey(TAB_VIEW), JSON.stringify({ x: pos.x, y: pos.y, width: size.width, height: size.height }));
  } catch {}
}

class TabsStore {
  /** Tabs in their own window, in the order they were torn off. Kept across starts. */
  torn = $state<ViewId[]>([]);

  constructor() {
    try {
      const saved = JSON.parse(localStorage.getItem(TORN_KEY) ?? "[]");
      if (Array.isArray(saved)) this.torn = saved.filter((v): v is ViewId => typeof v === "string" && tearable(v as ViewId));
    } catch {}
  }

  isTorn(view: ViewId) {
    return this.torn.includes(view);
  }

  async tearOff(view: ViewId, atCursor = false) {
    if (!tearable(view)) return;
    try {
      await invoke("tab_window_open", { view, bounds: savedBounds(view), atCursor });
    } catch (e) {
      build.error = String(e);
      return;
    }
    if (!this.isTorn(view)) this.set([...this.torn, view]);
    if (build.view === view) build.view = this.firstDocked();
  }

  show(view: ViewId) {
    void invoke("tab_window_open", { view, bounds: null }).catch(() => {});
  }

  dock(view: ViewId) {
    void invoke("tab_window_close", { view }).catch(() => {});
  }

  /** Reopens the windows that were open when the app last closed. */
  restore() {
    for (const view of this.torn) {
      void invoke("tab_window_open", { view, bounds: savedBounds(view) }).catch(() => this.closed(view));
    }
    if (this.isTorn(build.view)) build.view = this.firstDocked();
  }

  init() {
    void listen<ViewId>("tab-window:closed", (e) => this.closed(e.payload));
    void listen<ViewId>("tab-window:dock", (e) => {
      this.closed(e.payload);
      appOptions.open = false;
      build.view = e.payload;
    });
    void listen<{ view: ViewId; jump: Jump | null }>("tab:goto", (e) => goTo(e.payload.view, e.payload.jump));
    void listen<"save" | "save-as">("tab:command", (e) => {
      void getCurrentWindow().setFocus();
      void (e.payload === "save-as" ? build.saveAs() : build.save());
    });
  }

  private closed(view: ViewId) {
    if (this.isTorn(view)) this.set(this.torn.filter((v) => v !== view));
  }

  private firstDocked(): ViewId {
    return (["overview", ...tabList().map((t) => t.id)] as ViewId[]).find((v) => tearable(v) && !this.isTorn(v)) ?? "import";
  }

  private set(torn: ViewId[]) {
    this.torn = torn;
    try {
      localStorage.setItem(TORN_KEY, JSON.stringify(torn));
    } catch {}
  }
}

export const tabs = new TabsStore();

/** Shows `view`, in whichever window holds it, and hands it `jump` if one is given. */
export function goTo(view: ViewId, jump: Jump | null = null) {
  if (TAB_VIEW) {
    if (view === TAB_VIEW) {
      ui.jump = jump;
      return;
    }
    void emitTo("main", "tab:goto", { view, jump });
    void invoke("main_window_focus");
    return;
  }
  if (IS_MAIN_WINDOW && tabs.isTorn(view)) {
    if (jump) void emitTo(`tab-${view}`, "tab:jump", jump);
    tabs.show(view);
    return;
  }
  appOptions.open = false;
  ui.jump = jump;
  build.view = view;
}

/** Re-pulls the build whenever another window or the assistant changes it; `itemWindow.init` covers the item window. */
export async function followOtherWindows(): Promise<() => void> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const resync = () => {
    clearTimeout(timer);
    timer = setTimeout(() => void build.sync().catch(() => {}), 40);
  };
  const offs: UnlistenFn[] = [
    await listen<{ from?: string } | null>("build:edited", (e) => {
      if (e.payload?.from === WINDOW_LABEL) return;
      // The main window counts edits made anywhere, for the assistant's Undo warning.
      if (IS_MAIN_WINDOW) build.userEdits++;
      resync();
    }),
  ];
  if (TAB_VIEW) {
    offs.push(await listen("mcp:changed", resync));
    offs.push(await listen<Jump>("tab:jump", (e) => (ui.jump = e.payload)));
  }
  return () => {
    clearTimeout(timer);
    for (const off of offs) off();
  };
}
