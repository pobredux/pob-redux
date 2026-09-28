<script lang="ts">
  import { onDestroy, tick, untrack } from "svelte";
  import { engine, type BreakdownSection } from "$lib/engine.svelte";
  import {
    calcBreakdownKey,
    calcBreakdownWidth,
    chooseBreakdownPosition,
    clampBreakdownPosition,
    type CalcBreakdownRef,
  } from "$lib/calc-breakdown";
  import { m } from "$lib/paraglide/messages";
  import BreakdownPanel from "./BreakdownPanel.svelte";
  import Icon from "./Icon.svelte";
  import PobText from "./PobText.svelte";

  type BreakdownWindow = {
    sections: BreakdownSection[];
    title: string;
    key: string;
    ref: CalcBreakdownRef;
    x: number;
    y: number;
    width: number;
    height: number;
    ready: boolean;
  };
  type PinnedBreakdown = BreakdownWindow & {
    z: number;
    defaultWidth: number;
    defaultHeight: number;
    widthCustom: boolean;
    heightCustom: boolean;
  };
  type Props = { revision: number; onpinnedchange?: (keys: Set<string>) => void };

  let { revision, onpinnedchange }: Props = $props();
  let hover = $state.raw<BreakdownWindow | null>(null);
  let pinned = $state.raw<PinnedBreakdown[]>([]);
  let hoverElement = $state<HTMLElement>();
  const pinnedElements = new Map<string, HTMLElement>();
  const requests = new Map<string, number>();
  const cache = new Map<string, BreakdownSection[]>();
  let hoverTimer = 0;
  let hoverRequest = 0;
  let pinRequest = 0;
  let stopPointerTracking: (() => void) | null = null;

  const viewport = () => ({ width: window.innerWidth, height: window.innerHeight });
  const cacheKey = (key: string) => `${key}:${revision}`;
  const position = (node: HTMLElement, width: number, height = 0, excludeKey?: string) =>
    chooseBreakdownPosition(node.getBoundingClientRect(), width, height, viewport(), pinned, excludeKey);

  $effect(() => {
    revision;
    untrack(() => {
      clearTimeout(hoverTimer);
      cache.clear();
      hoverRequest++;
      hover = null;
      for (const popup of pinned) void fetchPinned(popup);
    });
  });

  onDestroy(() => {
    clearTimeout(hoverTimer);
    stopPointerTracking?.();
  });

  function notifyPinned() {
    onpinnedchange?.(new Set(pinned.map((popup) => popup.key)));
  }

  function registerPinned(node: HTMLElement, key: string) {
    pinnedElements.set(key, node);
    const observer = new ResizeObserver(() => {
      const height = node.getBoundingClientRect().height;
      updatePinned(key, (popup) => popup.heightCustom || popup.height === height
        ? popup
        : { ...popup, height, defaultHeight: height });
    });
    observer.observe(node);
    return {
      destroy() {
        observer.disconnect();
        pinnedElements.delete(key);
      },
    };
  }

  function updatePinned(key: string, update: (popup: PinnedBreakdown) => PinnedBreakdown) {
    const index = pinned.findIndex((popup) => popup.key === key);
    if (index < 0) return;
    const next = update(pinned[index]);
    if (next === pinned[index]) return;
    pinned = [...pinned.slice(0, index), next, ...pinned.slice(index + 1)];
  }

  function closePinned(key: string) {
    requests.delete(key);
    const closed = pinned.find((popup) => popup.key === key);
    pinned = pinned.filter((popup) => popup.key !== key)
      .map((popup) => closed && popup.z > closed.z ? { ...popup, z: popup.z - 1 } : popup);
    notifyPinned();
  }

  function isPinned(key: string) {
    return pinned.some((popup) => popup.key === key);
  }

  function bringToFront(key: string) {
    const target = pinned.find((popup) => popup.key === key);
    if (!target || target.z === pinned.length) return;
    pinned = pinned.map((popup) => popup.key === key
      ? { ...popup, z: pinned.length }
      : popup.z > target.z ? { ...popup, z: popup.z - 1 } : popup);
  }

  async function placeHover(node: HTMLElement, key: string) {
    await tick();
    if (!hover || hover.key !== key || !hoverElement) return;
    const height = hoverElement.getBoundingClientRect().height;
    hover = { ...hover, ...position(node, hover.width, height), height, ready: true };
  }

  async function measurePinned(key: string, node?: HTMLElement) {
    await tick();
    const popup = pinned.find((candidate) => candidate.key === key);
    const element = pinnedElements.get(key);
    if (!popup || !element) return;
    const height = element.getBoundingClientRect().height;
    const nextPosition = node
      ? position(node, popup.width, height, key)
      : clampBreakdownPosition(popup, popup.width, height, viewport());
    updatePinned(key, (current) => ({
      ...current,
      ...nextPosition,
      height,
      defaultHeight: current.heightCustom ? current.defaultHeight : height,
      ready: true,
    }));
  }

  function applyPinnedSections(key: string, title: string, sections: BreakdownSection[], node?: HTMLElement) {
    const width = calcBreakdownWidth(sections, title, window.innerWidth);
    updatePinned(key, (current) => ({
      ...current,
      sections,
      width: current.widthCustom ? current.width : width,
      defaultWidth: width,
      ready: false,
    }));
    void measurePinned(key, node);
  }

  async function fetchPinned(popup: PinnedBreakdown, node?: HTMLElement) {
    const request = ++pinRequest;
    requests.set(popup.key, request);
    try {
      const result = await engine.calcCellBreakdown(popup.ref);
      if (requests.get(popup.key) !== request || !isPinned(popup.key)) return;
      cache.set(cacheKey(popup.key), result.sections);
      applyPinnedSections(popup.key, popup.title, result.sections, node);
    } catch {
      if (requests.get(popup.key) === request) closePinned(popup.key);
    }
  }

  function clampPinnedWindows() {
    const bounds = viewport();
    let changed = false;
    const next = pinned.map((popup) => {
      const defaultWidth = calcBreakdownWidth(popup.sections, popup.title, bounds.width);
      const width = Math.min(popup.widthCustom ? popup.width : defaultWidth, Math.max(1, bounds.width - 24));
      const height = Math.min(popup.height, Math.max(1, Math.min(bounds.height * 0.6, bounds.height - 54)));
      const nextPosition = clampBreakdownPosition(popup, width, height, bounds);
      if (popup.defaultWidth === defaultWidth && popup.width === width && popup.height === height
        && popup.x === nextPosition.x && popup.y === nextPosition.y) return popup;
      changed = true;
      return { ...popup, ...nextPosition, width, height, defaultWidth };
    });
    if (changed) pinned = next;
  }

  function trackPointer(event: PointerEvent, move: (event: PointerEvent) => void) {
    stopPointerTracking?.();
    const target = event.currentTarget as HTMLElement;
    const pointerId = event.pointerId;
    const stop = () => {
      if (target.hasPointerCapture(pointerId)) target.releasePointerCapture(pointerId);
      target.removeEventListener("pointermove", move);
      target.removeEventListener("pointerup", stop);
      target.removeEventListener("pointercancel", stop);
      stopPointerTracking = null;
    };
    stopPointerTracking = stop;
    target.setPointerCapture(pointerId);
    target.addEventListener("pointermove", move);
    target.addEventListener("pointerup", stop, { once: true });
    target.addEventListener("pointercancel", stop, { once: true });
  }

  function startDrag(event: PointerEvent, key: string) {
    const target = event.target as HTMLElement;
    if (!target.closest(".bdhead") || target.closest("button")) return;
    const popup = pinned.find((candidate) => candidate.key === key);
    if (!popup) return;
    event.preventDefault();
    const offsetX = event.clientX - popup.x;
    const offsetY = event.clientY - popup.y;
    const move = (next: PointerEvent) => updatePinned(key, (current) => ({
      ...current,
      ...clampBreakdownPosition({ x: next.clientX - offsetX, y: next.clientY - offsetY }, current.width, current.height, viewport()),
    }));
    trackPointer(event, move);
  }

  function startResize(event: PointerEvent, key: string, corner: "nw" | "ne" | "sw" | "se") {
    event.preventDefault();
    event.stopPropagation();
    const popup = pinned.find((candidate) => candidate.key === key);
    if (!popup) return;
    bringToFront(key);
    const startX = event.clientX;
    const startY = event.clientY;
    const right = popup.x + popup.width;
    const bottom = popup.y + popup.height;
    const west = corner.includes("w");
    const north = corner.includes("n");
    const move = (next: PointerEvent) => updatePinned(key, (current) => {
      const bounds = viewport();
      const minWidth = Math.min(320, current.defaultWidth);
      const minHeight = Math.min(140, current.defaultHeight || 140);
      const maxWidth = west ? right - 12 : bounds.width - popup.x - 12;
      const maxHeight = Math.min(bounds.height * 0.6, north ? bottom - 42 : bounds.height - popup.y - 12);
      let width = Math.max(minWidth, Math.min(west ? popup.width - (next.clientX - startX) : popup.width + next.clientX - startX, maxWidth));
      let height = Math.max(minHeight, Math.min(north ? popup.height - (next.clientY - startY) : popup.height + next.clientY - startY, maxHeight));
      let widthCustom = true;
      let heightCustom = true;
      if (Math.abs(width - current.defaultWidth) <= 16 && current.defaultWidth <= maxWidth) {
        width = current.defaultWidth;
        widthCustom = false;
      }
      if (Math.abs(height - current.defaultHeight) <= 16 && current.defaultHeight <= maxHeight) {
        height = current.defaultHeight;
        heightCustom = false;
      }
      return {
        ...current,
        x: west ? right - width : popup.x,
        y: north ? bottom - height : popup.y,
        width,
        height,
        widthCustom,
        heightCustom,
      };
    });
    trackPointer(event, move);
  }

  function handlePointerDown(event: PointerEvent, key: string) {
    bringToFront(key);
    startDrag(event, key);
  }

  export function show(node: HTMLElement, ref: CalcBreakdownRef, title: string, pin: boolean) {
    clearTimeout(hoverTimer);
    const key = calcBreakdownKey(ref);
    const cached = cache.get(cacheKey(key));
    if (pin) {
      hoverRequest++;
      hover = null;
      if (isPinned(key)) {
        closePinned(key);
        return;
      }
      const width = Math.min(560, window.innerWidth - 24);
      const popup: PinnedBreakdown = {
        sections: [],
        title,
        key,
        ref,
        ...position(node, width),
        width,
        height: 0,
        defaultWidth: width,
        defaultHeight: 0,
        widthCustom: false,
        heightCustom: false,
        ready: false,
        z: pinned.length + 1,
      };
      pinned = [...pinned, popup];
      notifyPinned();
      if (cached) {
        applyPinnedSections(key, title, cached, node);
      } else {
        void fetchPinned(popup, node);
      }
      return;
    }
    if (isPinned(key)) return;
    const request = ++hoverRequest;
    const width = Math.min(560, window.innerWidth - 24);
    const apply = (sections: BreakdownSection[]) => {
      const fittedWidth = calcBreakdownWidth(sections, title, window.innerWidth);
      hover = { sections, title, key, ref, ...position(node, fittedWidth), width: fittedWidth, height: 0, ready: false };
      void placeHover(node, key);
    };
    if (cached) {
      apply(cached);
      return;
    }
    hover = null;
    hoverTimer = window.setTimeout(async () => {
      try {
        const result = await engine.calcCellBreakdown(ref);
        if (request !== hoverRequest) return;
        cache.set(cacheKey(key), result.sections);
        apply(result.sections);
      } catch {
        if (request === hoverRequest) hover = null;
      }
    }, 140);
  }

  export function leave() {
    clearTimeout(hoverTimer);
    hoverRequest++;
    hover = null;
  }
</script>

<svelte:window onresize={clampPinnedWindows} />

<div class="breakdown-layer">
  {#if hover}
    <aside
      bind:this={hoverElement}
      class="bdpop hover"
      class:ready={hover.ready}
      style:left={`${hover.x}px`}
      style:top={`${hover.y}px`}
      style:width={`${hover.width}px`}
      style:z-index={pinned.length + 1}
      role="tooltip"
    >
      <div class="bdhead">
        <span class="label"><PobText text={hover.title} calcs /></span>
      </div>
      <div class="bdscroll">
        <BreakdownPanel sections={hover.sections} />
      </div>
    </aside>
  {/if}

  {#each pinned as popup (popup.key)}
    <div
      use:registerPinned={popup.key}
      class="bdpop pinned"
      class:ready={popup.ready}
      style:left={`${popup.x}px`}
      style:top={`${popup.y}px`}
      style:width={`${popup.width}px`}
      style:height={popup.heightCustom ? `${popup.height}px` : undefined}
      style:z-index={popup.z}
      role="dialog"
      tabindex="-1"
      aria-label={popup.title}
      onpointerdown={(event) => handlePointerDown(event, popup.key)}
    >
      <div class="bdhead">
        <span class="bdtitle">
          <span class="bdpin"><Icon name="push-pin" size={12} /></span>
          <span class="label"><PobText text={popup.title} calcs /></span>
        </span>
        <button class="btn sm ghost" onclick={() => closePinned(popup.key)}>{m.common_close()}</button>
      </div>
      <div class="bdscroll">
        <BreakdownPanel sections={popup.sections} />
      </div>
      {#each ["nw", "ne", "sw", "se"] as corner}
        <button
          class={`resize-handle ${corner}`}
          tabindex="-1"
          aria-label={popup.title}
          onpointerdown={(event) => startResize(event, popup.key, corner as "nw" | "ne" | "sw" | "se")}
        ></button>
      {/each}
    </div>
  {/each}
</div>

<style>
  .breakdown-layer {
    position: fixed;
    inset: 0;
    z-index: 30;
    pointer-events: none;
  }
  .bdpop {
    position: fixed;
    box-sizing: border-box;
    max-width: calc(100vw - 24px);
    max-height: 60vh;
    display: flex;
    flex-direction: column;
    background: color-mix(in srgb, var(--bg-1) 96%, transparent);
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    box-shadow: var(--shadow-tooltip);
    backdrop-filter: blur(8px);
    pointer-events: none;
    visibility: hidden;
  }
  .bdpop.ready {
    visibility: visible;
  }
  .bdpop.pinned {
    pointer-events: auto;
  }
  .bdpop.pinned .bdhead {
    cursor: move;
    user-select: none;
  }
  .bdpop.pinned .bdhead button {
    cursor: pointer;
    position: relative;
    z-index: 2;
  }
  .bdhead {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 8px 12px;
    border-bottom: 1px solid var(--line-0);
    border-radius: var(--r-2) var(--r-2) 0 0;
    background: var(--bg-3);
    color: var(--fg-0);
    font-weight: 700;
  }
  .bdtitle {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 7px;
  }
  .bdpin {
    flex: none;
    display: grid;
    place-items: center;
    color: var(--focus);
  }
  .bdscroll {
    min-height: 0;
    overflow-y: auto;
    padding: 10px 12px;
  }
  .resize-handle {
    appearance: none;
    position: absolute;
    width: 16px;
    height: 16px;
    padding: 0;
    border: 0;
    background: transparent;
    z-index: 1;
  }
  .resize-handle::after {
    content: "";
    position: absolute;
    width: 5px;
    height: 5px;
    opacity: 0;
    transition: opacity 80ms linear;
  }
  .bdpop:hover .resize-handle::after {
    opacity: 0.75;
  }
  .resize-handle.nw {
    top: 0;
    left: 0;
    cursor: nwse-resize;
  }
  .resize-handle.ne {
    top: 0;
    right: 0;
    cursor: nesw-resize;
  }
  .resize-handle.sw {
    bottom: 0;
    left: 0;
    cursor: nesw-resize;
  }
  .resize-handle.se {
    right: 0;
    bottom: 0;
    cursor: nwse-resize;
  }
  .resize-handle.nw::after,
  .resize-handle.sw::after {
    left: 3px;
    border-left: 1px solid var(--line-2);
  }
  .resize-handle.ne::after,
  .resize-handle.se::after {
    right: 3px;
    border-right: 1px solid var(--line-2);
  }
  .resize-handle.nw::after,
  .resize-handle.ne::after {
    top: 3px;
    border-top: 1px solid var(--line-2);
  }
  .resize-handle.sw::after,
  .resize-handle.se::after {
    bottom: 3px;
    border-bottom: 1px solid var(--line-2);
  }
</style>
