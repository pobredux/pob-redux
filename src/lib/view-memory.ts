import { untrack } from "svelte";
import type { Attachment } from "svelte/attachments";
import { build } from "$lib/state/build.svelte";
import { game } from "$lib/state/game.svelte";

const memory = new Map<string, { scope: string; value: unknown }>();

/** State remembered under one loaded build is never restored into another. */
export const buildScope = () => untrack(() => `${game.current}|${build.info?.generation ?? ""}|${build.info?.file ?? ""}`);

export function recall<T>(key: string, scope: string): T | undefined {
  const m = memory.get(key);
  return m && m.scope === scope ? (m.value as T) : undefined;
}

export function remember(key: string, scope: string, value: unknown) {
  memory.set(key, { scope, value });
}

const RESTORE_MS = 1500;

/** Retries for RESTORE_MS because a view's content often arrives after it mounts. */
export function keepScroll(key: string, perBuild = true): Attachment<HTMLElement> {
  return (node) => {
    const scope = perBuild ? buildScope() : "";
    const saved = recall<{ top: number; left: number }>(`scroll:${key}`, scope);
    let top = saved?.top ?? 0;
    let left = saved?.left ?? 0;
    let frame = 0;
    const until = performance.now() + RESTORE_MS;
    const restore = () => {
      frame = 0;
      node.scrollTop = top;
      node.scrollLeft = left;
      const there = Math.abs(node.scrollTop - top) <= 1 && Math.abs(node.scrollLeft - left) <= 1;
      if (!there && performance.now() < until) frame = requestAnimationFrame(restore);
    };
    const stop = () => {
      cancelAnimationFrame(frame);
      frame = 0;
    };
    const track = () => {
      if (frame) return;
      top = node.scrollTop;
      left = node.scrollLeft;
    };
    if (top || left) frame = requestAnimationFrame(restore);
    node.addEventListener("scroll", track, { passive: true });
    node.addEventListener("wheel", stop, { passive: true });
    node.addEventListener("pointerdown", stop);
    node.addEventListener("keydown", stop);
    return () => {
      stop();
      node.removeEventListener("scroll", track);
      node.removeEventListener("wheel", stop);
      node.removeEventListener("pointerdown", stop);
      node.removeEventListener("keydown", stop);
      remember(`scroll:${key}`, scope, { top, left });
    };
  };
}
