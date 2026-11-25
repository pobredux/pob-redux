<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { emitTo, listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { status as engineStatus } from "$lib/engine.svelte";
  import { ui } from "$lib/state/ui.svelte";
  import ViewHost from "$lib/components/ViewHost.svelte";
  import ConfirmModal from "$lib/components/ConfirmModal.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import LogoMark from "$lib/components/LogoMark.svelte";
  import Tooltip from "$lib/components/Tooltip.svelte";
  import { build, type ViewId } from "$lib/state/build.svelte";
  import { game } from "$lib/state/game.svelte";
  import { itemWindow } from "$lib/state/item-window.svelte";
  import { followOtherWindows, rememberBounds, tabList } from "$lib/state/tabs.svelte";
  import { tooltip } from "$lib/state/tooltip.svelte";
  import { isTextField, modKey, shortcutKey } from "$lib/keys";
  import { TAB_VIEW } from "$lib/window";
  import { m } from "$lib/paraglide/messages";

  const view = TAB_VIEW as ViewId;
  const win = getCurrentWindow();
  const tab = $derived(tabList().find((t) => t.id === view));
  let ready = $state(false);
  let maximized = $state(false);

  $effect(() => {
    const name = build.info?.name;
    win.setTitle(`${tab?.label ?? view}${name ? ` — ${name}` : ""} — PoB Redux`).catch(() => {});
  });

  async function engineReady() {
    for (;;) {
      const s = await engineStatus().catch(() => null);
      if (s?.state === "ready") return;
      await new Promise((r) => setTimeout(r, 100));
    }
  }

  async function dock() {
    await rememberBounds();
    await emitTo("main", "tab-window:dock", view).catch(() => {});
    void invoke("tab_window_close", { view });
  }

  async function close() {
    await rememberBounds();
    void invoke("tab_window_close", { view });
  }

  function onKey(e: KeyboardEvent) {
    if (!modKey(e) || e.altKey) return;
    const key = shortcutKey(e).toLowerCase();
    if (key === "s") {
      e.preventDefault();
      void emitTo("main", "tab:command", e.shiftKey ? "save-as" : "save");
    } else if (e.shiftKey) {
      return;
    } else if ((key === "z" || key === "y") && view === "tree" && !isTextField(e.target)) {
      e.preventDefault();
      void (key === "z" ? build.undo() : build.redo());
    } else if (key === "=" || key === "+") {
      e.preventDefault();
      ui.stepScale(1);
    } else if (key === "-") {
      e.preventDefault();
      ui.stepScale(-1);
    } else if (key === "0") {
      e.preventDefault();
      ui.setScale(1);
    }
  }

  onMount(() => {
    const offTips = tooltip.install();
    const offs: (() => void)[] = [];
    let boundsTimer = 0;
    const keepBounds = () => {
      clearTimeout(boundsTimer);
      boundsTimer = window.setTimeout(() => void rememberBounds(), 400);
    };
    void (async () => {
      offs.push(await followOtherWindows());
      offs.push(await listen("game:changed", () => location.reload()));
      offs.push(await win.onMoved(keepBounds));
      offs.push(
        await win.onResized(async () => {
          maximized = await win.isMaximized();
          keepBounds();
        }),
      );
      itemWindow.init();
      void rememberBounds();
      maximized = await win.isMaximized();
      await game.init().catch(() => {});
      await engineReady();
      build.view = view;
      await build.sync().catch((e) => (build.error = String(e)));
      ready = true;
    })();
    window.addEventListener("keydown", onKey);
    return () => {
      clearTimeout(boundsTimer);
      window.removeEventListener("keydown", onKey);
      offTips();
      for (const off of offs) off();
    };
  });
</script>

<div class="win">
  <header class="bar" data-tauri-drag-region>
    <LogoMark class="mark" />
    {#if tab}<span class="icon" data-tauri-drag-region><Icon name={tab.icon} size={14} /></span>{/if}
    <span class="name" data-tauri-drag-region>{tab?.label ?? view}</span>
    {#if build.info}<span class="build" data-tauri-drag-region>{build.info.name}{build.info.unsaved ? " •" : ""}</span>{/if}
    <div class="grow" data-tauri-drag-region></div>
    <button class="wc" title={m.tab_window_dock_title()} aria-label={m.item_window_dock()} onclick={dock}>
      <Icon name="arrow-square-in" size={15} />
    </button>
    <button class="wc" aria-label={m.titlebar_minimize()} onclick={() => win.minimize()}>
      <svg width="10" height="10" viewBox="0 0 10 10"><path d="M0 5.5h10" stroke="currentColor" stroke-width="1" /></svg>
    </button>
    <button class="wc" aria-label={m.titlebar_maximize()} onclick={() => win.toggleMaximize()}>
      {#if maximized}
        <svg width="10" height="10" viewBox="0 0 10 10"><path d="M2.5 0.5h7v7M0.5 2.5h7v7h-7z" fill="none" stroke="currentColor" stroke-width="1" /></svg>
      {:else}
        <svg width="10" height="10" viewBox="0 0 10 10"><rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" stroke-width="1" /></svg>
      {/if}
    </button>
    <button class="wc close" aria-label={m.common_close()} onclick={close}>
      <svg width="10" height="10" viewBox="0 0 10 10"><path d="M0.5 0.5l9 9M9.5 0.5l-9 9" stroke="currentColor" stroke-width="1.1" /></svg>
    </button>
  </header>
  <main class="view">
    {#if ready && build.loaded}
      <ViewHost {view} />
    {:else}
      <div class="wait dim">{m.tab_window_loading()}</div>
    {/if}
  </main>
  {#if build.error}
    <button class="strip err" onclick={() => build.clearError()} title={m.status_dismiss()}>{build.error}</button>
  {:else if build.notice}
    <div class="strip ok">{build.notice}</div>
  {/if}
  <ConfirmModal />
  <Tooltip />
</div>

<style>
  .win {
    position: relative;
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--bg-0);
  }
  .bar {
    flex: none;
    height: var(--titlebar-h);
    display: flex;
    align-items: stretch;
    gap: 10px;
    padding-left: 12px;
    background: linear-gradient(90deg, var(--glow), transparent 55%), var(--bg-1);
    border-bottom: 1px solid var(--line-0);
    -webkit-app-region: drag;
    user-select: none;
  }
  .bar :global(.mark) {
    align-self: center;
    width: 15px;
    height: 15px;
    flex: none;
    color: var(--fg-0);
    pointer-events: none;
  }
  .icon {
    align-self: center;
    display: grid;
    color: var(--fg-2);
  }
  .name {
    align-self: center;
    font-size: var(--fs-sm);
    font-weight: 600;
    letter-spacing: 0.06em;
    color: var(--fg-0);
  }
  .build {
    align-self: center;
    min-width: 0;
    overflow: hidden;
    font-size: var(--fs-xs);
    color: var(--fg-2);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .grow {
    flex: 1;
  }
  .wc {
    appearance: none;
    width: 42px;
    display: grid;
    place-items: center;
    border: 0;
    background: transparent;
    color: var(--fg-2);
    cursor: var(--cursor-arrow, default);
    -webkit-app-region: no-drag;
  }
  .wc:hover {
    background: var(--bg-hover);
    color: var(--fg-0);
  }
  .wc.close:hover {
    background: #c42b1c;
    color: #fff;
  }
  .view {
    flex: 1;
    min-width: 0;
    min-height: 0;
    position: relative;
    display: flex;
    flex-direction: column;
  }
  .wait {
    margin: auto;
    font-size: var(--fs-sm);
  }
  .strip {
    flex: none;
    padding: 4px 12px;
    border: 0;
    border-top: 1px solid var(--line-0);
    background: var(--bg-1);
    font: inherit;
    font-size: var(--fs-xs);
    text-align: left;
  }
  .strip.err {
    color: var(--bad);
    cursor: var(--cursor-pointer, pointer);
  }
  .strip.ok {
    color: var(--ok);
  }
</style>
