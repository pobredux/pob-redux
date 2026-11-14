import manifest from "./assets/cursors/cursors.json";

const files = import.meta.glob<string>("./assets/cursors/*/*.png", { eager: true, query: "?url", import: "default" });

export type CursorState = "arrow" | "hover" | "click";

export const CURSORS: string[] = manifest.map((c) => c.id);

export function cursorImage(id: string, state: CursorState, retina = false): string | undefined {
  return files[`./assets/cursors/${id}/${state}${retina ? "@2x" : ""}.png`];
}

/** A CSS `cursor` value for a game cursor, or null when the skin is unknown. */
export function cursorValue(id: string, state: CursorState, fallback: string): string | null {
  const skin = manifest.find((c) => c.id === id);
  const one = cursorImage(id, state);
  const two = cursorImage(id, state, true);
  if (!skin || !one || !two) return null;
  const [x, y] = skin.hotspot;
  // The prefixed form is the one both WebView2 and WebKit accept in `cursor`.
  return `-webkit-image-set(url("${one}") 1x, url("${two}") 2x) ${x} ${y}, ${fallback}`;
}
