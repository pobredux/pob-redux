import { Channel, invoke } from "@tauri-apps/api/core";
import { build } from "$lib/state/build.svelte";
import { game } from "$lib/state/game.svelte";
import { locale } from "$lib/state/locale.svelte";
import { m } from "$lib/paraglide/messages";

const KEY = "pob-redux:voice";
const DEFAULT_MODEL = "base.en";
export const HISTORY = 240;

export interface VoiceModel {
  id: string;
  bytes: number;
  english: boolean;
  installed: boolean;
}

interface VoiceStatus {
  models: VoiceModel[];
  languages: string[];
}

export const appSpeech = () => locale.current.split("-")[0].toLowerCase();

/** Names from the open build, so Whisper spells them the way PoB does. */
export function vocabulary() {
  const names = new Set<string>();
  const info = build.info;
  if (info) {
    names.add(info.className);
    if (info.ascendClassName) names.add(info.ascendClassName);
  }
  for (const g of build.skills?.socketGroups ?? []) {
    if (g.grantedBy?.item) names.add(g.grantedBy.item);
    for (const gem of g.gems) if (gem.name) names.add(gem.name);
  }
  const text = `${game.isPoe1 ? "Path of Exile" : "Path of Exile 2"} build. ${[...names].join(", ")}.`;
  return text.length > 600 ? text.slice(0, text.lastIndexOf(",", 600)) + "." : text;
}

class VoiceStore {
  enabled = $state(false);
  model = $state(DEFAULT_MODEL);
  /** A Whisper language code or "auto"; null follows the interface language. */
  language = $state<string | null>(null);
  status = $state<VoiceStatus | null>(null);
  phase = $state<"idle" | "recording" | "transcribing">("idle");
  level = $state(0);
  levels = $state<number[]>([]);
  since = $state<number | null>(null);
  /** The wake listener owns this recording, not voice_start. */
  wake = $state(false);
  progress = $state<number | null>(null);
  error = $state<string | null>(null);
  private started = false;
  private opening: Promise<unknown> = Promise.resolve();

  get current(): VoiceModel | undefined {
    return this.status?.models.find((x) => x.id === this.model);
  }

  get ready() {
    return this.enabled && !!this.current?.installed;
  }

  get speech() {
    if (this.current?.english) return "en";
    const lang = this.language ?? appSpeech();
    return lang === "auto" || this.status?.languages.includes(lang) ? lang : "auto";
  }

  async init() {
    if (this.started) return;
    this.started = true;
    try {
      const saved = JSON.parse(localStorage.getItem(KEY) ?? "{}");
      this.enabled = saved.enabled === true;
      if (typeof saved.model === "string") this.model = saved.model;
      if (typeof saved.language === "string") this.language = saved.language;
    } catch {}
    await this.refresh();
    if (this.status && !this.current) this.model = DEFAULT_MODEL;
  }

  async refresh() {
    this.status = await invoke<VoiceStatus>("voice_status").catch(() => null);
  }

  private persist() {
    try {
      localStorage.setItem(KEY, JSON.stringify({ enabled: this.enabled, model: this.model, language: this.language }));
    } catch {}
  }

  setEnabled(on: boolean) {
    this.enabled = on;
    this.persist();
  }

  setModel(id: string) {
    if (this.phase !== "idle" || this.progress !== null) return;
    this.error = null;
    this.model = id;
    this.persist();
  }

  setLanguage(code: string | null) {
    this.language = code;
    this.persist();
  }

  async install() {
    this.error = null;
    this.progress = 0;
    const channel = new Channel<{ done: number; total: number }>();
    channel.onmessage = (p) => (this.progress = Math.floor((p.done * 100) / p.total));
    try {
      await invoke("voice_install", { model: this.model, onProgress: channel });
    } catch (e) {
      this.error = String(e);
    } finally {
      this.progress = null;
      await this.refresh();
    }
  }

  async remove() {
    this.error = null;
    try {
      await invoke("voice_remove", { model: this.model });
    } catch (e) {
      this.error = String(e);
    }
    await this.refresh();
  }

  async start() {
    if (this.phase !== "idle") return;
    this.error = null;
    this.phase = "recording";
    this.levels = [];
    this.since = Date.now();
    const channel = new Channel<number>();
    channel.onmessage = (l) => {
      if (this.phase === "recording") this.pushLevel(l);
    };
    this.opening = invoke("voice_start", { model: this.model, onLevel: channel }).catch((e) => {
      this.phase = "idle";
      this.error = String(e);
    });
    await this.opening;
  }

  /** Stops recording and returns the text, or "" when nothing was said. */
  async stop(): Promise<string> {
    if (this.phase !== "recording") return "";
    await this.opening;
    if (this.phase !== "recording") return "";
    this.phase = "transcribing";
    this.level = 0;
    try {
      const text = await invoke<string>("voice_stop", { model: this.model, language: this.speech, prompt: vocabulary() });
      if (!text) this.error = m.chat_voice_none();
      return text;
    } catch (e) {
      this.error = String(e);
      return "";
    } finally {
      this.phase = "idle";
    }
  }

  pushLevel(l: number) {
    this.level = l;
    this.levels = [...this.levels.slice(1 - HISTORY), l];
  }

  async cancel() {
    if (this.phase !== "recording") return;
    if (this.wake) {
      this.phase = "idle";
      this.wake = false;
      this.level = 0;
      await invoke("wake_cancel").catch(() => {});
      return;
    }
    await this.opening;
    this.phase = "idle";
    this.level = 0;
    await invoke("voice_cancel").catch(() => {});
  }
}

export const voice = new VoiceStore();
