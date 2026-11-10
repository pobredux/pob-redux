import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { WeaponSetMode } from "$lib/engine.svelte";

const KEY = "pob-redux:ui";
/** Must match the window's minimum size in tauri.conf.json. */
const LAYOUT_MIN_W = 1100;
const LAYOUT_MIN_H = 680;

export type Dock = "top" | "bottom";
/** Something the command palette asked a view to show; the view clears it once shown. */
export type Jump =
  | { view: "tree"; node: number; name: string }
  | { view: "items"; item: number }
  | { view: "skills"; group: number }
  | { view: "config"; label: string }
  | { view: "calcs"; label: string };
export const THEMES = ["system", "dark", "light", "wraeclast", "abyss", "breach", "ritual", "delirium"] as const;
export type Theme = (typeof THEMES)[number];
export const SCALE_MIN = 0.75;
export const SCALE_MAX = 2;
export const SCALE_STEP = 0.1;
export const CONTRAST_MAX = 100;
/** How far each grey may travel toward --fg-0 at full lift, so the ramp keeps its order. */
const CONTRAST_REACH = [0.8, 0.7, 0.65, 0.6];
/** What the OS asking for more contrast is worth, matching the old More step. */
const CONTRAST_SYSTEM = 25;

class UiStore {
  sidebarCollapsed = $state(false);
  treeBarDock = $state<Dock>("top");
  theme = $state<Theme>("system");
  contrastAuto = $state(true);
  contrastLevel = $state(0);
  scale = $state(1);
  scaleApplied = $state(1);
  treeWeaponSet = $state<WeaponSetMode>(0);
  treeStatDiff = $state(true);
  treeAscCentre = $state(true);
  tabIcons = $state(false);
  paletteOpen = $state(false);
  newBuildOpen = $state(false);
  jump = $state<Jump | null>(null);

  private systemLight = window.matchMedia("(prefers-color-scheme: light)");
  private systemContrast = window.matchMedia("(prefers-contrast: more)");

  constructor() {
    try {
      const saved = JSON.parse(localStorage.getItem(KEY) ?? "{}");
      if (typeof saved.sidebarCollapsed === "boolean") this.sidebarCollapsed = saved.sidebarCollapsed;
      if (saved.treeBarDock === "top" || saved.treeBarDock === "bottom") this.treeBarDock = saved.treeBarDock;
      if ((THEMES as readonly string[]).includes(saved.theme)) this.theme = saved.theme;
      const named = typeof saved.contrast === "string" ? saved.contrast : null;
      if (typeof saved.contrastAuto === "boolean") this.contrastAuto = saved.contrastAuto;
      else if (named) this.contrastAuto = named === "system";
      if (typeof saved.contrastLevel === "number") this.contrastLevel = clampContrast(saved.contrastLevel);
      // The three steps this replaced, at the levels that reproduce them.
      else if (named) this.contrastLevel = named === "most" ? 40 : named === "more" ? CONTRAST_SYSTEM : 0;
      if (typeof saved.scale === "number") this.scale = clampScale(saved.scale);
      if (typeof saved.treeStatDiff === "boolean") this.treeStatDiff = saved.treeStatDiff;
      if (typeof saved.treeAscCentre === "boolean") this.treeAscCentre = saved.treeAscCentre;
      if (typeof saved.tabIcons === "boolean") this.tabIcons = saved.tabIcons;
    } catch {}
    this.applyTheme();
    this.systemLight.addEventListener("change", () => this.applyTheme());
    this.systemContrast.addEventListener("change", () => this.applyContrast());
    void this.applyScale();
    let timer = 0;
    const refit = () => {
      clearTimeout(timer);
      timer = window.setTimeout(() => void this.applyScale(), 120);
    };
    const win = getCurrentWindow();
    win.onResized(refit).catch(() => {});
    win.onScaleChanged(refit).catch(() => {});
  }

  get scaleLimited() {
    return this.scaleApplied < this.scale;
  }

  toggleSidebar() {
    this.sidebarCollapsed = !this.sidebarCollapsed;
    this.save();
  }

  setTreeStatDiff(show: boolean) {
    this.treeStatDiff = show;
    this.save();
  }

  setTreeAscCentre(centre: boolean) {
    this.treeAscCentre = centre;
    this.save();
  }

  setTabIcons(always: boolean) {
    this.tabIcons = always;
    this.save();
  }

  setTreeBarDock(dock: Dock) {
    this.treeBarDock = dock;
    this.save();
  }

  setTheme(theme: Theme) {
    this.theme = theme;
    this.applyTheme();
    this.save();
  }

  /** The lift actually in force, which Auto takes from the OS. */
  get contrastEffective() {
    return this.contrastAuto ? (this.systemContrast.matches ? CONTRAST_SYSTEM : 0) : this.contrastLevel;
  }

  setContrastAuto(auto: boolean) {
    this.contrastAuto = auto;
    this.applyContrast();
    this.save();
  }

  setContrastLevel(level: number) {
    this.contrastLevel = clampContrast(level);
    this.applyContrast();
    this.save();
  }

  setScale(scale: number) {
    this.scale = clampScale(scale);
    void this.applyScale();
    this.save();
  }

  stepScale(direction: 1 | -1) {
    this.setScale(this.scale + direction * SCALE_STEP);
  }

  private applyTheme() {
    const effective = this.theme === "system" ? (this.systemLight.matches ? "light" : "dark") : this.theme;
    document.documentElement.dataset.theme = effective;
    // Each theme has its own ramp, so the lift has to be recomputed with it.
    this.applyContrast();
  }

  private applyContrast() {
    const el = document.documentElement;
    const level = this.contrastEffective;
    for (let i = 1; i <= 4; i++) {
      if (level <= 0) {
        el.style.removeProperty(`--fg-${i}`);
        continue;
      }
      // Mixed in oklab so each step of the slider lifts by the same perceived
      // amount, and against the tokens so it follows a theme change on its own.
      const pct = (level / 100) * CONTRAST_REACH[i - 1] * 100;
      el.style.setProperty(`--fg-${i}`, `color-mix(in oklab, var(--fg-0) ${pct.toFixed(1)}%, var(--ramp-${i}))`);
    }
  }

  private async applyScale() {
    let target = this.scale;
    if (target > 1) {
      try {
        const win = getCurrentWindow();
        const size = (await win.innerSize()).toLogical(await win.scaleFactor());
        const fit = Math.min(size.width / LAYOUT_MIN_W, size.height / LAYOUT_MIN_H);
        target = Math.max(1, Math.min(target, Math.floor(fit * 20) / 20));
      } catch {}
    }
    if (target === this.scaleApplied) return;
    this.scaleApplied = target;
    await getCurrentWebview()
      .setZoom(target)
      .catch(() => {});
  }

  private save() {
    try {
      localStorage.setItem(
        KEY,
        JSON.stringify({
          sidebarCollapsed: this.sidebarCollapsed,
          treeBarDock: this.treeBarDock,
          theme: this.theme,
          contrastAuto: this.contrastAuto,
          contrastLevel: this.contrastLevel,
          scale: this.scale,
          treeStatDiff: this.treeStatDiff,
          treeAscCentre: this.treeAscCentre,
          tabIcons: this.tabIcons,
        }),
      );
    } catch {}
  }
}

function clampContrast(n: number) {
  return Math.min(CONTRAST_MAX, Math.max(0, Math.round(n / 5) * 5));
}

function clampScale(s: number) {
  const snapped = Math.round(s / 0.05) * 0.05;
  return Math.min(SCALE_MAX, Math.max(SCALE_MIN, Number(snapped.toFixed(2))));
}

export const ui = new UiStore();
