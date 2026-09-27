<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import { build, type ViewId } from "$lib/state/build.svelte";
  import { ui } from "$lib/state/ui.svelte";
  import { appOptions } from "$lib/state/options.svelte";
  import { game, GAMES, GAME_SHORT, GAME_LABEL, type Game } from "$lib/state/game.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { m } from "$lib/paraglide/messages";

  import LogoMark from "$lib/components/LogoMark.svelte";

  const win = getCurrentWindow();
  let maximized = $state(false);
  let tabsEl = $state<HTMLDivElement | null>(null);

  function openView(id: ViewId) {
    appOptions.open = false;
    build.view = id;
  }

  // Declining the unsaved-changes prompt leaves the game as it was, which the
  // select cannot know, so put it back.
  async function onGame(e: Event) {
    const el = e.currentTarget as HTMLSelectElement;
    if (!(await game.choose(el.value as Game))) el.value = game.current;
  }

  $effect(() => {
    void build.view;
    void appOptions.open;
    tabsEl?.querySelector(".tab.active")?.scrollIntoView({ block: "nearest", inline: "nearest" });
  });

  const tabs = $derived<{ id: ViewId; label: string; key: string }[]>([
    { id: "import", label: m.view_builds(), key: "1" },
    { id: "tree", label: m.view_tree(), key: "2" },
    { id: "skills", label: m.view_skills(), key: "3" },
    { id: "items", label: m.view_items(), key: "4" },
    { id: "calcs", label: m.view_calcs(), key: "5" },
    { id: "config", label: m.view_config(), key: "6" },
    { id: "notes", label: m.view_notes(), key: "7" },
    { id: "party", label: m.view_party(), key: "8" },
    { id: "optimise", label: m.view_optimise(), key: "9" },
    { id: "compare", label: m.view_compare(), key: "0" },
  ]);

  // the build name lives in the sidebar; the OS title carries it for the taskbar
  $effect(() => {
    const name = build.info?.name;
    win.setTitle(name ? `${name}${build.info?.unsaved ? " •" : ""} — PoB Redux` : "PoB Redux").catch(() => {});
  });

  onMount(() => {
    win.isMaximized().then((v) => (maximized = v));
    const un = win.onResized(async () => (maximized = await win.isMaximized()));
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.shiftKey && !e.altKey && e.key.toLowerCase() === "s") {
        void build.saveAs();
        e.preventDefault();
        return;
      }
      if (e.ctrlKey && !e.shiftKey && !e.altKey) {
        if (e.key === "s") {
          void build.save();
          e.preventDefault();
        }
        const t = tabs.find((t) => t.key === e.key);
        if (t && build.loaded) {
          openView(t.id);
          e.preventDefault();
        }
        if (e.key === "z") {
          build.undo();
          e.preventDefault();
        }
        if (e.key === "y") {
          build.redo();
          e.preventDefault();
        }
        if (e.key === "b") {
          ui.toggleSidebar();
          e.preventDefault();
        }
        if (e.key === ",") {
          appOptions.open = !appOptions.open;
          e.preventDefault();
        }
        if (e.key === "=" || e.key === "+") {
          ui.stepScale(1);
          e.preventDefault();
        }
        if (e.key === "-") {
          ui.stepScale(-1);
          e.preventDefault();
        }
        if (e.key === "0") {
          ui.setScale(1);
          e.preventDefault();
        }
      }
    };
    window.addEventListener("keydown", onKey);
    const onWheel = (e: WheelEvent) => {
      if (!tabsEl || tabsEl.scrollWidth <= tabsEl.clientWidth || !e.deltaY) return;
      tabsEl.scrollLeft += e.deltaY;
      e.preventDefault();
    };
    tabsEl?.addEventListener("wheel", onWheel, { passive: false });
    return () => {
      tabsEl?.removeEventListener("wheel", onWheel);
      un.then((f) => f());
      window.removeEventListener("keydown", onKey);
    };
  });
</script>

<header class="titlebar" data-tauri-drag-region>
  <div class="brand" class:wide={!ui.sidebarCollapsed} data-tauri-drag-region>
    <LogoMark class="mark" />
    <span class="name">PoB <span class="thin">Redux</span></span>
    <button
      class="sb"
      class:on={!ui.sidebarCollapsed}
      aria-label={ui.sidebarCollapsed ? m.titlebar_sidebar_show() : m.titlebar_sidebar_hide()}
      title={ui.sidebarCollapsed ? m.titlebar_sidebar_show_title() : m.titlebar_sidebar_hide_title()}
      onclick={() => ui.toggleSidebar()}
    >
      <svg width="14" height="12" viewBox="0 0 14 12" fill="none" stroke="currentColor" stroke-width="1">
        <rect x="0.5" y="0.5" width="13" height="11" rx="1.5" />
        <path d="M5 0.5v11" />
      </svg>
    </button>
  </div>

  <div class="game">
    <select
      class="select gsel"
      aria-label={m.titlebar_game()}
      value={game.current}
      disabled={game.switching || build.busy > 0}
      title={GAME_LABEL[game.current]}
      onchange={onGame}
    >
      {#each GAMES as g (g)}
        <option value={g}>{GAME_SHORT[g]}</option>
      {/each}
    </select>
  </div>

  <div class="tabs" role="tablist" bind:this={tabsEl}>
    {#each tabs as t (t.id)}
      <button
        role="tab"
        class="tab"
        class:active={build.view === t.id && !appOptions.open}
        aria-selected={build.view === t.id && !appOptions.open}
        disabled={!build.loaded && t.id !== "import"}
        onclick={() => openView(t.id)}
        title={`Ctrl+${t.key}`}
      >
        {t.label}
      </button>
    {/each}
  </div>

  <div class="spacer" data-tauri-drag-region></div>

  <div class="controls">
    <button
      class="wc opts"
      class:on={appOptions.open}
      aria-label={m.titlebar_settings()}
      aria-pressed={appOptions.open}
      title={m.titlebar_settings_title()}
      onclick={() => (appOptions.open = !appOptions.open)}
    >
      <Icon name="gear" size={15} />
    </button>
    <button class="wc" aria-label={m.titlebar_minimize()} onclick={() => win.minimize()}>
      <svg width="10" height="10" viewBox="0 0 10 10"><path d="M0 5.5h10" stroke="currentColor" stroke-width="1" /></svg>
    </button>
    <button class="wc" aria-label={m.titlebar_maximize()} onclick={() => win.toggleMaximize()}>
      {#if maximized}
        <svg width="10" height="10" viewBox="0 0 10 10"
          ><path d="M2.5 0.5h7v7M0.5 2.5h7v7h-7z" fill="none" stroke="currentColor" stroke-width="1" /></svg
        >
      {:else}
        <svg width="10" height="10" viewBox="0 0 10 10"><rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" stroke-width="1" /></svg>
      {/if}
    </button>
    <button class="wc close" aria-label={m.titlebar_close()} onclick={() => win.close()}>
      <svg width="10" height="10" viewBox="0 0 10 10"><path d="M0.5 0.5l9 9M9.5 0.5l-9 9" stroke="currentColor" stroke-width="1.1" /></svg>
    </button>
  </div>
</header>

<style>
  .titlebar {
    height: var(--titlebar-h);
    display: grid;
    grid-template-columns: auto auto minmax(0, auto) minmax(0, 1fr) auto;
    align-items: stretch;
    background: var(--bg-1);
    border-bottom: 1px solid var(--line-0);
    -webkit-app-region: drag;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 10px 0 14px;
    border-right: 1px solid var(--line-0);
  }
  .brand.wide {
    width: var(--sidebar-w);
  }
  .sb {
    appearance: none;
    border: 0;
    background: transparent;
    color: var(--fg-3);
    margin-left: auto;
    padding: 4px;
    border-radius: var(--r-1);
    display: grid;
    place-items: center;
    cursor: pointer;
    -webkit-app-region: no-drag;
  }
  .sb:hover {
    color: var(--fg-0);
    background: var(--bg-hover);
  }
  .sb.on {
    color: var(--fg-2);
  }
  .brand :global(.mark) {
    width: 16px;
    height: 16px;
    flex: none;
    color: var(--fg-0);
    pointer-events: none;
    user-select: none;
  }
  .name {
    font-size: var(--fs-sm);
    font-weight: 600;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--fg-0);
  }
  .thin {
    font-weight: 400;
    color: var(--fg-2);
  }
  .game {
    display: flex;
    align-items: center;
    padding: 0 10px;
    border-right: 1px solid var(--line-0);
    -webkit-app-region: no-drag;
  }
  .gsel {
    height: 22px;
    /* Fixed, so a longer option cannot widen the closed control. */
    width: 68px;
    padding: 0 20px 0 7px;
    background-color: transparent;
    background-position: right 6px center;
    border-color: transparent;
    color: var(--fg-0);
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    letter-spacing: 0.06em;
  }
  .gsel:hover:not(:disabled) {
    background-color: var(--bg-hover);
  }
  .gsel:disabled {
    cursor: default;
    opacity: 0.6;
  }
  .gsel option {
    background: var(--bg-1);
    color: var(--fg-0);
  }
  .tabs {
    display: flex;
    align-items: stretch;
    min-width: 0;
    overflow-x: auto;
    scrollbar-width: none;
    -webkit-app-region: no-drag;
  }
  .tabs::-webkit-scrollbar {
    display: none;
  }
  .tab {
    flex: none;
    white-space: nowrap;
    appearance: none;
    border: 0;
    border-right: 1px solid var(--line-0);
    background: transparent;
    color: var(--fg-2);
    font-size: var(--fs-sm);
    padding: 0 16px;
    cursor: pointer;
    position: relative;
    letter-spacing: 0.01em;
  }
  .tab:hover:not(:disabled) {
    color: var(--fg-0);
    background: var(--bg-2);
  }
  .tab.active {
    color: var(--fg-0);
    background: var(--bg-0);
  }
  .tab.active::after {
    content: "";
    position: absolute;
    left: 0;
    right: 0;
    bottom: -1px;
    height: 1px;
    background: var(--bg-0);
  }
  .tab.active::before {
    content: "";
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    height: 1px;
    background: var(--accent);
  }
  .tab:disabled {
    color: var(--fg-4);
    cursor: default;
  }
  .spacer {
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 0;
  }
  .controls {
    display: flex;
    -webkit-app-region: no-drag;
  }
  .wc {
    appearance: none;
    width: 46px;
    border: 0;
    background: transparent;
    color: var(--fg-2);
    cursor: default;
    display: grid;
    place-items: center;
  }
  .wc:hover {
    background: var(--bg-hover);
    color: var(--fg-0);
  }
  .wc.opts.on {
    color: var(--fg-0);
    background: var(--bg-active);
  }
  .wc.opts {
    width: 40px;
    margin-right: 6px;
    border-right: 1px solid var(--line-0);
    cursor: pointer;
  }
  .wc.close:hover {
    background: #c42b1c;
    color: #fff;
  }
  @media (max-width: 1360px) {
    .brand.wide {
      width: auto;
    }
    .name {
      display: none;
    }
    .tab {
      padding: 0 11px;
    }
  }
</style>
