<script lang="ts">
  import { poolStatus, telemetry, type EngineStatus, type AppPaths, type PoolStatus } from "$lib/engine.svelte";
  import { build } from "$lib/state/build.svelte";
  import { mcp } from "$lib/state/mcp.svelte";
  import { chat } from "$lib/state/chat.svelte";
  import { appOptions } from "$lib/state/options.svelte";
  import { game } from "$lib/state/game.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { m } from "$lib/paraglide/messages";

  let { status, paths }: { status: EngineStatus | null; paths: AppPaths | null } = $props();

  const DISCORD_URL = "https://discord.pobredux.com/";

  // Workers spawn on demand and boot in the background; poll only while one is
  // booting, and look again whenever engine activity starts or stops.
  let pool = $state<PoolStatus | null>(null);
  $effect(() => {
    if (status?.state !== "ready") return;
    void telemetry.inflight;
    let timer = 0;
    const tick = async () => {
      try {
        pool = await poolStatus();
      } catch {
        pool = null;
        return;
      }
      if (pool.spawned > pool.ready) timer = window.setTimeout(tick, 1500);
    };
    tick();
    return () => clearTimeout(timer);
  });

  const stateColor = $derived(
    status?.state === "ready" ? "var(--ok)" : status?.state === "error" ? "var(--bad)" : "var(--warn)",
  );
</script>

<footer class="statusbar">
  <div class="seg">
    <span class="dot" style:background={stateColor} class:pulse={status?.state === "booting" || telemetry.inflight > 0}></span>
    <span>{m.status_engine({ state: status?.state ?? "…" })}</span>
    {#if status?.boot_ms != null}<span class="dim num">{m.status_boot_ms({ ms: status.boot_ms })}</span>{/if}
  </div>
  {#if pool && pool.size > 0}
    <div class="seg dim" title={m.status_workers_title()}>
      <span>{m.status_workers()}</span>
      <span class="num" class:pulse={pool.spawned > pool.ready}>{pool.ready}/{pool.size}</span>
    </div>
  {/if}
  {#if paths?.sync}
    <div class="seg">
      <span class="dim">PoB</span>
      <span class="num">{paths.sync.upstream_version}</span>
      <span class="dim num" title={paths.sync.upstream_commit}>{paths.sync.upstream_commit.slice(0, 8)}</span>
    </div>
  {/if}
  {#if mcp.status?.running}
    <div class="seg" title={m.status_mcp_title()}>
      <span class="dim">mcp</span>
      <span class="num">:{mcp.status.port}</span>
    </div>
  {/if}
  <div class="grow"></div>
  {#if build.error}
    <button class="seg err" onclick={() => build.clearError()} title={m.status_dismiss()}>
      <span>{build.error}</span>
    </button>
  {:else if build.notice}
    <div class="seg ok"><span>{build.notice}</span></div>
  {/if}
  {#if telemetry.lastMethod}
    <div class="seg dim">
      <span class="mono">{telemetry.lastMethod}</span>
      <span class="num">{telemetry.lastMs.toFixed(1)} ms</span>
    </div>
  {/if}
  {#if build.info}
    <div class="seg dim"><span>{m.status_rev()}</span><span class="num">{build.info.rev}</span></div>
  {/if}
  {#if game.isPoe2}
    <button
      class="seg iconbtn"
      class:on={chat.open && !appOptions.open}
      onclick={() => {
        if (appOptions.open && chat.open) appOptions.open = false;
        else chat.toggle();
      }}
      title={m.status_assistant_title()}
      aria-label={m.status_assistant()}
    >
      <Icon name="chat-text" size={15} />
    </button>
  {/if}
  <button
    class="seg iconbtn"
    onclick={() => openUrl(DISCORD_URL).catch(() => {})}
    title={m.status_discord_title()}
    aria-label={m.status_discord()}
  >
    <Icon name="discord-logo" size={15} />
  </button>
</footer>

<style>
  .statusbar {
    height: var(--statusbar-h);
    display: flex;
    align-items: stretch;
    background: var(--bg-1);
    border-top: 1px solid var(--line-0);
    font-size: var(--fs-xs);
    color: var(--fg-2);
    letter-spacing: 0.02em;
  }
  .seg {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 10px;
    border-right: 1px solid var(--line-0);
    white-space: nowrap;
    appearance: none;
    background: none;
    border-top: 0;
    border-bottom: 0;
    border-left: 0;
    color: inherit;
    font: inherit;
  }
  .grow {
    flex: 1;
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
  }
  .pulse {
    animation: pulse 1.2s ease-in-out infinite;
  }
  /* `.seg` is a flex row with side padding meant for text. For a lone icon that
     leaves it off-axis, so centre it explicitly on a fixed width. */
  .iconbtn {
    justify-content: center;
    width: 34px;
    padding: 0;
    border-left: 1px solid var(--line-0);
    border-right: 0;
    color: var(--fg-2);
  }
  .iconbtn:hover:not(:disabled) {
    color: var(--fg-0);
    background: var(--bg-2);
  }
  .err {
    color: var(--bad);
    border-left: 1px solid var(--line-0);
    cursor: pointer;
    max-width: 50vw;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ok {
    color: var(--ok);
    border-left: 1px solid var(--line-0);
    max-width: 50vw;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .seg.on {
    color: var(--fg-0);
    background: var(--bg-hover);
  }
</style>
