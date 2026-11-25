export const isMac = /mac/i.test(navigator.userAgent);

/** Ctrl, or Cmd on macOS. */
export function modKey(e: KeyboardEvent | MouseEvent): boolean {
  return isMac ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey;
}

const US_PUNCTUATION: Record<string, [string, string]> = {
  BracketLeft: ["[", "{"],
  BracketRight: ["]", "}"],
  Semicolon: [";", ":"],
  Quote: ["'", '"'],
  Comma: [",", "<"],
  Period: [".", ">"],
  Slash: ["/", "?"],
  Backquote: ["`", "~"],
};

/** `e.key`, except that a key typing a non-Latin character (Cyrillic, Greek) reads as the US key in its place. */
export function shortcutKey(e: KeyboardEvent): string {
  if (e.key.length !== 1 || e.key.charCodeAt(0) < 128) return e.key;
  const letter = /^Key([A-Z])$/.exec(e.code)?.[1];
  if (letter) return e.shiftKey ? letter : letter.toLowerCase();
  return US_PUNCTUATION[e.code]?.[e.shiftKey ? 1 : 0] ?? e.key;
}

const NON_TEXT_INPUTS = new Set(["checkbox", "radio", "button", "submit", "reset", "range", "color", "file", "image"]);

export function isTextField(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable || target instanceof HTMLTextAreaElement) return true;
  return target instanceof HTMLInputElement && !NON_TEXT_INPUTS.has(target.type);
}

/** The keycaps for a shortcut written "Mod+Shift+K". */
export function keycaps(shortcut: string): string[] {
  return shortcut.split("+").map((k) => {
    if (k === "Mod") return isMac ? "⌘" : "Ctrl";
    if (k === "Shift") return isMac ? "⇧" : "Shift";
    if (k === "Alt") return isMac ? "⌥" : "Alt";
    return k;
  });
}
