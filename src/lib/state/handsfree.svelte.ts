import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { chat } from "$lib/state/chat.svelte";
import { voice, vocabulary } from "$lib/state/voice.svelte";
import { IS_ASSISTANT_WINDOW, IS_MAIN_WINDOW } from "$lib/window";
import { m } from "$lib/paraglide/messages";

const KEY = "pob-redux:handsfree";

export interface TtsVoice {
  id: string;
  name: string;
  installed: boolean;
}

interface TtsStatus {
  installed: boolean;
  bytes: number;
  voices: TtsVoice[];
}

interface WakeStatus {
  running: boolean;
  profile: number | null;
}

export type SetupEvent =
  | { stage: "ask"; attempt: number; total: number; test: boolean }
  | { stage: "result"; attempt: number; total: number; test: boolean; voice: boolean; heard: boolean; personal: boolean | null; best: number }
  | { stage: "read"; needed: number }
  | { stage: "reading"; voiced: number; needed: number }
  | { stage: "fitted"; min_positive: number; max_negative: number; threshold: number; separated: boolean }
  | { stage: "done"; generic: number; personal: number; tries: number }
  | { stage: "failed"; error: string };

/** Markdown to plain sentences, with the names Phonon would otherwise mispronounce spelled out. */
export function speakable(md: string): string {
  let t = md
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/`([^`]*)`/g, "$1")
    .replace(/!\[[^\]]*\]\([^)]*\)/g, "")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .split("\n")
    .filter((l) => !/^\s*\|/.test(l))
    .join("\n")
    .replace(/^\s{0,3}#{1,6}\s*/gm, "")
    .replace(/^\s*(?:[-*+]|\d+\.)\s+/gm, "")
    .replace(/[*_~]+/g, "")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/\bPoB Redux\b/gi, "P.O.B. Ree-ducks")
    .replace(/\bRedux\b/gi, "Ree-ducks")
    .replace(/\bPoB\b/g, "P.O.B.")
    .replace(/\bPoE ?2\b/gi, "Path of Exile 2")
    .replace(/\bPoE ?1\b/gi, "Path of Exile 1")
    .replace(/\bPoE\b/g, "Path of Exile")
    .replace(/\bDPS\b/g, "D.P.S.")
    .replace(/\bEHP\b/g, "effective H.P.");
  if (t.length > 900) {
    const cut = t.slice(0, 900);
    const end = Math.max(cut.lastIndexOf(". "), cut.lastIndexOf("! "), cut.lastIndexOf("? "));
    t = end > 200 ? cut.slice(0, end + 1) : `${cut}…`;
  }
  return t;
}

const WAKE = /^\s*(?:[\p{L}']+[\s,.!]+)?(?:re?\s?ducks?|re?dux|dux|riddick'?s?|redox)\b[\s,.!?:-]*/iu;

/** The capture starts 1.5 s before the trigger, so a real call is transcribed with the wake phrase first. */
export function splitWake(text: string): { heard: boolean; question: string } {
  let rest = text.trim();
  let heard = false;
  while (WAKE.test(rest)) {
    heard = true;
    rest = rest.replace(WAKE, "").trim();
  }
  return { heard, question: rest.charAt(0).toUpperCase() + rest.slice(1) };
}

function chime() {
  try {
    const ctx = new AudioContext();
    const gain = ctx.createGain();
    gain.connect(ctx.destination);
    [880, 1320].forEach((f, i) => {
      const o = ctx.createOscillator();
      o.frequency.value = f;
      o.connect(gain);
      const t = ctx.currentTime + i * 0.09;
      gain.gain.setValueAtTime(0.08, t);
      o.start(t);
      o.stop(t + 0.08);
    });
    setTimeout(() => void ctx.close(), 500);
  } catch {}
}

class HandsFree {
  wake = $state(false);
  speak = $state(false);
  voiceId = $state("default");
  state = $state<"off" | "listening" | "capturing" | "paused" | "setup" | "stopped" | "error">("off");
  profile = $state<number | null>(null);
  tts = $state<TtsStatus | null>(null);
  progress = $state<number | null>(null);
  error = $state<string | null>(null);
  setup = $state<SetupEvent[]>([]);
  speaking = $state(false);
  switching = $state(false);
  private started = false;

  get listening() {
    return this.state === "listening" || this.state === "capturing" || this.state === "paused" || this.state === "setup";
  }

  /** The chat lives in the assistant window while it is popped out, so only that window answers. */
  private get owner() {
    return IS_ASSISTANT_WINDOW || (IS_MAIN_WINDOW && !chat.poppedOut);
  }

  async init() {
    if (this.started) return;
    this.started = true;
    try {
      const saved = JSON.parse(localStorage.getItem(KEY) ?? "{}");
      this.wake = saved.wake === true;
      this.speak = saved.speak === true;
      if (typeof saved.voice === "string") this.voiceId = saved.voice;
    } catch {}
    await listen<string>("wake:state", (e) => (this.state = e.payload as HandsFree["state"]));
    await listen<SetupEvent>("wake:setup", (e) => (this.setup = [...this.setup, e.payload]));
    await listen("wake:triggered", () => this.owner && this.onTrigger());
    await listen<number>("wake:level", (e) => {
      if (this.owner && voice.wake) voice.pushLevel(e.payload);
    });
    await listen("wake:captured", () => this.owner && void this.onCaptured());
    await listen("wake:cancelled", () => {
      if (!this.owner) return;
      voice.phase = "idle";
      voice.wake = false;
    });
    await this.refresh();
    if (IS_MAIN_WINDOW && this.wake) await this.startListening();
  }

  async refresh() {
    const [wake, tts] = await Promise.all([
      invoke<WakeStatus>("wake_status").catch(() => null),
      invoke<TtsStatus>("tts_status").catch(() => null),
    ]);
    if (wake) {
      this.profile = wake.profile;
      if (!wake.running && this.state !== "error") this.state = "off";
    }
    this.tts = tts;
  }

  private persist() {
    try {
      localStorage.setItem(KEY, JSON.stringify({ wake: this.wake, speak: this.speak, voice: this.voiceId }));
    } catch {}
  }

  private async startListening() {
    this.error = null;
    await voice.init();
    if (!voice.current?.installed) {
      this.error = m.wake_needs_model();
      return;
    }
    try {
      await invoke("wake_start");
      this.state = "listening";
    } catch (e) {
      this.error = String(e);
      this.state = "error";
    }
  }

  async setWake(on: boolean) {
    this.wake = on;
    this.persist();
    if (on) await this.startListening();
    else {
      await invoke("wake_stop").catch(() => {});
      this.state = "off";
    }
  }

  setSpeak(on: boolean) {
    this.speak = on;
    this.persist();
  }

  async setVoice(id: string) {
    this.error = null;
    if (this.speaking) this.stopSpeaking();
    const previous = this.voiceId;
    this.voiceId = id;
    this.persist();
    if (id === "default" || this.tts?.voices.find((v) => v.id === id)?.installed) return;
    this.switching = true;
    try {
      await invoke("tts_voice_install", { voice: id });
      await this.refresh();
    } catch (e) {
      this.voiceId = previous;
      this.persist();
      this.error = String(e);
    } finally {
      this.switching = false;
    }
  }

  async installTts() {
    this.error = null;
    this.progress = 0;
    const channel = new Channel<{ done: number; total: number }>();
    channel.onmessage = (p) => (this.progress = Math.floor((p.done * 100) / p.total));
    try {
      await invoke("tts_install", { onProgress: channel });
    } catch (e) {
      this.error = String(e);
    } finally {
      this.progress = null;
      await this.refresh();
    }
  }

  async removeTts() {
    this.error = null;
    await invoke("tts_remove").catch((e) => (this.error = String(e)));
    await this.refresh();
  }

  async say(markdown: string) {
    const text = speakable(markdown);
    if (!text || !this.tts?.installed) return;
    this.speaking = true;
    try {
      await invoke("tts_speak", { text, voice: this.voiceId });
    } catch (e) {
      this.error = String(e);
    } finally {
      this.speaking = false;
    }
  }

  stopSpeaking() {
    void invoke("tts_stop");
    this.speaking = false;
  }

  async startSetup() {
    this.error = null;
    this.setup = [];
    await invoke("wake_setup_start", { tries: 8, readSeconds: 12 });
  }

  nextSetup() {
    void invoke("wake_setup_next");
  }

  async cancelSetup() {
    await invoke("wake_setup_cancel").catch(() => {});
    this.setup = [];
    if (!this.wake) await invoke("wake_stop").catch(() => {});
  }

  async saveSetup() {
    await invoke("wake_setup_save");
    this.setup = [];
    await this.refresh();
    if (!this.wake) await invoke("wake_stop").catch(() => {});
  }

  async resetProfile() {
    await invoke("wake_profile_reset").catch((e) => (this.error = String(e)));
    await this.refresh();
  }

  private onTrigger() {
    if (IS_MAIN_WINDOW && !chat.open) chat.toggle();
    voice.error = null;
    voice.levels = [];
    voice.since = Date.now();
    voice.wake = true;
    voice.phase = "recording";
    chime();
  }

  private async onCaptured() {
    voice.phase = "transcribing";
    voice.level = 0;
    try {
      const { heard, question: text } = splitWake(await invoke<string>("wake_transcribe", { model: voice.model, language: voice.speech, prompt: `Hey Redux. ${vocabulary()}` }));
      voice.phase = "idle";
      voice.wake = false;
      if (!heard) {
        voice.error = m.wake_not_heard();
        return;
      }
      if (!text) {
        voice.error = m.chat_voice_none();
        return;
      }
      if (chat.busy) {
        chat.input = text;
        return;
      }
      chat.input = text;
      await chat.send({ spoken: true });
      const reply = chat.turns.at(-1);
      if (this.speak && reply?.kind === "assistant") await this.say(reply.text);
    } catch (e) {
      voice.error = String(e);
    } finally {
      voice.phase = "idle";
      voice.wake = false;
      await invoke("wake_resume").catch(() => {});
    }
  }
}

export const handsfree = new HandsFree();
