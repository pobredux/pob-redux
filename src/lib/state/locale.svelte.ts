import {
  baseLocale,
  locales,
  isLocale,
  overwriteGetLocale,
  getTextDirection,
  localStorageKey,
  type Locale,
} from "$lib/paraglide/runtime";

export type LocalePreference = "system" | Locale;

export const LOCALES = locales as readonly Locale[];

/** Each language in its own name, which is what a language picker should show. */
export const LOCALE_LABEL: Record<Locale, string> = {
  en: "English",
  de: "Deutsch",
  es: "Español",
  fr: "Français",
  ja: "日本語",
  ko: "한국어",
  "pt-BR": "Português (Brasil)",
  ru: "Русский",
  th: "ไทย",
  "zh-CN": "简体中文",
  "zh-TW": "繁體中文",
};

/** Tags the OS may report that no shipped locale matches exactly. Variant
 *  subtags (e.g. zh-Hant-TW) are matched per segment below. */
const ALIASES: Record<string, Locale> = {
  pt: "pt-BR", "pt-pt": "pt-BR",
  "zh-hant": "zh-TW", "zh-hk": "zh-TW", "zh-mo": "zh-TW",
  hant: "zh-TW",
};

function match(tag: string): Locale | undefined {
  const lower = tag.toLowerCase();
  const exact = LOCALES.find((l) => l.toLowerCase() === lower);
  if (exact) return exact;
  if (ALIASES[lower]) return ALIASES[lower];
  const parts = lower.split("-");
  // Browsers and Windows may report a script subtag; resolve it before the
  // bare-language fallback, which would otherwise pick the first zh locale.
  for (const part of parts) if (ALIASES[part]) return ALIASES[part];
  const base = parts[0];
  return LOCALES.find((l) => l.toLowerCase().split("-")[0] === base) ?? ALIASES[base];
}

function detectSystem(): Locale {
  for (const tag of navigator?.languages ?? []) {
    const hit = match(tag);
    if (hit) return hit;
  }
  return baseLocale as Locale;
}

/** `getLocale()` reads `current`, so every message re-runs on a change and the app switches without a reload. */
class LocaleStore {
  preference = $state<LocalePreference>("system");
  system = $state<Locale>(baseLocale as Locale);
  current = $state<Locale>(baseLocale as Locale);

  constructor() {
    let saved: string | null = null;
    try {
      saved = localStorage.getItem(localStorageKey);
    } catch {}
    this.system = detectSystem();
    this.preference = isLocale(saved) ? saved : "system";
    this.current = this.preference === "system" ? this.system : this.preference;
    overwriteGetLocale(() => this.current);
    this.applyDocument();
  }

  set(preference: LocalePreference) {
    this.preference = preference;
    this.current = preference === "system" ? this.system : preference;
    try {
      if (preference === "system") localStorage.removeItem(localStorageKey);
      else localStorage.setItem(localStorageKey, preference);
    } catch {}
    this.applyDocument();
  }

  private applyDocument() {
    document.documentElement.lang = this.current;
    document.documentElement.dir = getTextDirection(this.current);
  }
}

export const locale = new LocaleStore();
