<script lang="ts">
  import { fade } from "svelte/transition";
  import { treeArt } from "$lib/state/tree-art.svelte";
  import { gameData } from "$lib/state/game-data.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { m } from "$lib/paraglide/messages";

  void treeArt.init();
  void gameData.init();

  type Toast = { key: string; kind: "downloading" | "failed" | "done"; title: string; done?: number; total?: number; detail?: string };

  const toast = $derived.by((): Toast | null => {
    const art = treeArt.status;
    const data = gameData.status;
    if (art?.state === "downloading") return { key: "art", kind: "downloading", title: m.download_tree_art(), done: art.done, total: art.total };
    if (art?.state === "failed") return { key: "art-failed", kind: "failed", title: m.download_tree_art_failed(), detail: m.download_tree_art_failed_detail() };
    if (data?.state === "downloading") return { key: "data", kind: "downloading", title: m.download_game_data(), done: data.done, total: data.total };
    if (data?.state === "failed") return { key: "data-failed", kind: "failed", title: m.download_game_data_failed(), detail: m.download_game_data_failed_detail() };
    if (gameData.justDone) return { key: "data-done", kind: "done", title: m.download_game_data_done() };
    if (treeArt.justDone) return { key: "art-done", kind: "done", title: m.download_tree_art_done() };
    return null;
  });

  let dismissed = $state<string | null>(null);
  const mb = (bytes: number) => (bytes / 1048576).toFixed(1);
</script>

{#if toast && toast.key !== dismissed}
  <div class="toast" class:failed={toast.kind === "failed"} role="status" aria-live="polite" transition:fade={{ duration: 150 }}>
    <div class="head">
      {#if toast.kind === "done"}<span class="ok"><Icon name="check" size={13} /></span>{/if}
      <span class="title">{toast.title}</span>
      {#if toast.kind === "downloading" && toast.total}
        <span class="num">{mb(toast.done ?? 0)} / {mb(toast.total)} MB</span>
      {/if}
      <button class="close" aria-label={m.download_dismiss()} title={m.download_dismiss()} onclick={() => (dismissed = toast.key)}>
        <Icon name="x" size={12} />
      </button>
    </div>
    {#if toast.kind === "downloading"}
      <div class="track"><div class="fill" style:width="{toast.total ? ((toast.done ?? 0) / toast.total) * 100 : 0}%"></div></div>
      <div class="detail">{m.download_once()}</div>
    {:else if toast.kind === "failed"}
      <div class="detail">{toast.detail}</div>
      <div class="actions">
        <button
          class="btn sm"
          onclick={() => {
            dismissed = null;
            treeArt.retry();
          }}>{m.download_retry()}</button
        >
      </div>
    {/if}
  </div>
{/if}

<style>
  .toast {
    position: fixed;
    right: 12px;
    bottom: calc(var(--statusbar-h) + 12px);
    z-index: 30;
    width: 360px;
    display: grid;
    gap: 8px;
    padding: 10px 12px;
    background: var(--bg-1);
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    box-shadow: var(--shadow-pop);
    font-size: var(--fs-xs);
    color: var(--fg-1);
  }
  .toast.failed {
    border-left: 2px solid var(--warn);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .title {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--fg-0);
  }
  .failed .title {
    color: var(--warn);
  }
  .ok {
    display: inline-flex;
    color: var(--ok);
  }
  .num {
    font-family: var(--font-mono);
    color: var(--fg-1);
  }
  .close {
    display: inline-flex;
    padding: 2px;
    border: 0;
    background: none;
    color: var(--fg-2);
    cursor: pointer;
  }
  .close:hover {
    color: var(--fg-0);
  }
  .track {
    height: 4px;
    border-radius: 2px;
    background: var(--bg-3);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--focus);
    transition: width 120ms linear;
  }
  .detail {
    color: var(--fg-2);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
  }
</style>
