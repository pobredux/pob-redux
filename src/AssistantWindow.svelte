<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import "$lib/state/ui.svelte";
  import ChatPanel from "$lib/components/ChatPanel.svelte";
  import Tooltip from "$lib/components/Tooltip.svelte";
  import LogoMark from "$lib/components/LogoMark.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { tooltip } from "$lib/state/tooltip.svelte";
  import { chat } from "$lib/state/chat.svelte";
  import { build } from "$lib/state/build.svelte";
  import { game } from "$lib/state/game.svelte";
  import { m } from "$lib/paraglide/messages";

  const win = getCurrentWindow();
  let ready = $state(false);

  onMount(() => {
    const offTips = tooltip.install();
    const offs: (() => void)[] = [];
    const mirror = () => void build.mirror().catch(() => {});
    void (async () => {
      // Closing the window, by its button or the OS, hands the conversation back first.
      offs.push(
        await win.onCloseRequested((e) => {
          e.preventDefault();
          void chat.dock();
        }),
      );
      offs.push(await listen("mcp:changed", mirror));
      offs.push(
        await listen("build:edited", () => {
          build.userEdits++;
          mirror();
        }),
      );
      offs.push(
        await listen("assistant:switch-game", async () => {
          await game.init().catch(() => {});
          mirror();
          await chat.switchGame();
        }),
      );
      await game.init().catch(() => {});
      await build.mirror().catch(() => {});
      await chat.initWindow();
      ready = true;
    })();
    return () => {
      offTips();
      for (const off of offs) off();
    };
  });
</script>

<div class="win">
  <header class="bar" data-tauri-drag-region>
    <LogoMark class="mark" />
    <span class="name" data-tauri-drag-region>{m.assistant_window_title()}</span>
    {#if build.info}<span class="build" data-tauri-drag-region>{build.info.name}</span>{/if}
    <div class="grow" data-tauri-drag-region></div>
    <button class="wc" title={m.chat_dock_title()} aria-label={m.chat_dock()} disabled={chat.busy} onclick={() => chat.dock()}>
      <Icon name="arrow-square-in" size={15} />
    </button>
    <button class="wc" aria-label={m.titlebar_minimize()} onclick={() => win.minimize()}>
      <svg width="10" height="10" viewBox="0 0 10 10"><path d="M0 5.5h10" stroke="currentColor" stroke-width="1" /></svg>
    </button>
    <button class="wc close" title={m.chat_dock_title()} aria-label={m.chat_dock()} onclick={() => chat.dock()}>
      <svg width="10" height="10" viewBox="0 0 10 10"><path d="M0.5 0.5l9 9M9.5 0.5l-9 9" stroke="currentColor" stroke-width="1.1" /></svg>
    </button>
  </header>
  {#if ready}
    <div class="body">
      <ChatPanel windowed />
    </div>
  {/if}
  <Tooltip />
</div>

<style>
  .win {
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--bg-1);
  }
  .body {
    flex: 1;
    min-height: 0;
    display: flex;
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
  .wc:hover:not(:disabled) {
    background: var(--bg-hover);
    color: var(--fg-0);
  }
  .wc:disabled {
    opacity: 0.4;
  }
  .wc.close:hover {
    background: #c42b1c;
    color: #fff;
  }
</style>
