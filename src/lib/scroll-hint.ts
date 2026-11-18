import type { Attachment } from "svelte/attachments";

const FADE_PX = 48;
const themeListeners = new Set<() => void>();
let themeWatch: MutationObserver | null = null;

const isTransparent = (colour: string) => !colour || colour === "transparent" || colour === "rgba(0, 0, 0, 0)" || /\/\s*0\)$/.test(colour);

function panelBackground(node: Element): string | null {
  for (let el: Element | null = node; el; el = el.parentElement) {
    const colour = getComputedStyle(el).backgroundColor;
    if (!isTransparent(colour)) return colour;
  }
  return null;
}

function watchTheme(listener: () => void) {
  themeListeners.add(listener);
  themeWatch ??= new MutationObserver(() => themeListeners.forEach((fn) => fn()));
  themeWatch.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme", "class", "style"] });
  return () => {
    themeListeners.delete(listener);
    if (!themeListeners.size) {
      themeWatch?.disconnect();
      themeWatch = null;
    }
  };
}

/** Drives the `.scroll-hint` fade: `--scroll-more` is 1 while more is below and eases to 0 over the last 48px. */
export const scrollHint: Attachment<HTMLElement> = (node) => {
  let frame = 0;
  let childrenDirty = true;
  let colourDirty = true;
  const resize = new ResizeObserver(() => schedule());
  const update = () => {
    frame = 0;
    if (childrenDirty) {
      childrenDirty = false;
      resize.disconnect();
      resize.observe(node);
      for (const child of node.children) resize.observe(child);
    }
    if (colourDirty) {
      colourDirty = false;
      const colour = panelBackground(node);
      if (colour) node.style.setProperty("--scroll-fade-bg", colour);
      else node.style.removeProperty("--scroll-fade-bg");
      node.style.setProperty("--scroll-pad", getComputedStyle(node).paddingBottom);
    }
    const remaining = node.scrollHeight - node.clientHeight - node.scrollTop;
    node.style.setProperty("--scroll-more", remaining > 1 ? String(Math.min(1, remaining / FADE_PX)) : "0");
  };
  const schedule = () => {
    if (!frame) frame = requestAnimationFrame(update);
  };
  const mutations = new MutationObserver(() => {
    childrenDirty = true;
    schedule();
  });
  mutations.observe(node, { childList: true, subtree: true });
  node.addEventListener("scroll", schedule, { passive: true });
  const unwatchTheme = watchTheme(() => {
    colourDirty = true;
    schedule();
  });
  schedule();
  return () => {
    cancelAnimationFrame(frame);
    resize.disconnect();
    mutations.disconnect();
    unwatchTheme();
    node.removeEventListener("scroll", schedule);
    node.style.removeProperty("--scroll-more");
    node.style.removeProperty("--scroll-fade-bg");
    node.style.removeProperty("--scroll-pad");
  };
};
