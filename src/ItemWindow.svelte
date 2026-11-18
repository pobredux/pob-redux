<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { emit, listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import "$lib/state/ui.svelte";
  import BuySimilarDialog from "$lib/components/BuySimilarDialog.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import ItemDetailHead from "$lib/components/ItemDetailHead.svelte";
  import ItemDetailPane from "$lib/components/ItemDetailPane.svelte";
  import LogoMark from "$lib/components/LogoMark.svelte";
  import Tooltip from "$lib/components/Tooltip.svelte";
  import { saveTemplate } from "$lib/shared-items";
  import { build } from "$lib/state/build.svelte";
  import { game } from "$lib/state/game.svelte";
  import { tooltip } from "$lib/state/tooltip.svelte";
  import { m } from "$lib/paraglide/messages";

  const win = getCurrentWindow();
  let ready = $state(false);
  let itemId = $state<number | null>(null);
  let rev = $state(0);
  let missing = $state(false);
  let saved = $state<number | null>(null);
  let buying = $state(false);
  let transitioning = $state(false);

  async function refresh() {
    await build.mirror().catch(() => {});
    saved = null;
    rev++;
  }

  onMount(() => {
    const offTips = tooltip.install();
    const offs: (() => void)[] = [];
    void (async () => {
      offs.push(
        await listen<number>("item-window:show", (event) => {
          itemId = event.payload;
          missing = false;
          saved = null;
        }),
      );
      offs.push(await listen("build:edited", refresh));
      offs.push(await listen("mcp:changed", refresh));
      await game.init().catch(() => {});
      await build.mirror().catch(() => {});
      itemId = (await invoke<number | null>("item_window_current").catch(() => null)) ?? null;
      ready = true;
    })();
    return () => {
      offTips();
      for (const off of offs) off();
    };
  });

  function dock() {
    if (itemId != null) void emit("item-window:dock", itemId);
    void invoke("item_window_close");
  }

  async function save() {
    if (itemId == null) return;
    const id = itemId;
    if (await saveTemplate(id)) saved = id;
  }
</script>

<div class="win">
  <header class="bar" data-tauri-drag-region>
    <LogoMark class="mark" />
    <span class="name" data-tauri-drag-region>{m.item_window_title()}</span>
    {#if build.info}<span class="build" data-tauri-drag-region>{build.info.name}</span>{/if}
    <div class="grow" data-tauri-drag-region></div>
    <button class="wc" title={m.item_window_dock_title()} aria-label={m.item_window_dock()} onclick={dock}>
      <Icon name="arrow-square-in" size={15} />
    </button>
    <button class="wc" aria-label={m.titlebar_minimize()} onclick={() => win.minimize()}>
      <svg width="10" height="10" viewBox="0 0 10 10"><path d="M0 5.5h10" stroke="currentColor" stroke-width="1" /></svg>
    </button>
    <button class="wc close" aria-label={m.common_close()} onclick={() => invoke("item_window_close")}>
      <svg width="10" height="10" viewBox="0 0 10 10"><path d="M0.5 0.5l9 9M9.5 0.5l-9 9" stroke="currentColor" stroke-width="1.1" /></svg>
    </button>
  </header>
  {#if ready && build.info && itemId != null}
    {#if missing}
      <p class="missing">{m.item_window_missing()}</p>
    {:else}
      <ItemDetailHead saved={saved === itemId} busy={transitioning} onsave={save} onbuy={() => (buying = true)} />
      <ItemDetailPane
        {itemId}
        generation={build.info.generation}
        {rev}
        sync={false}
        bind:transitioning
        onedited={() => {
          void emit("item:edited");
          void refresh();
        }}
        onmissing={() => (missing = true)}
      />
    {/if}
  {/if}
  {#if buying && itemId != null}
    <BuySimilarDialog target={{ itemId }} onclose={() => (buying = false)} />
  {/if}
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
  .missing {
    padding: 16px 14px;
    color: var(--fg-2);
    font-size: var(--fs-sm);
  }
</style>
