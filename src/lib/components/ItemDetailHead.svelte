<script lang="ts">
  import Icon, { type IconName } from "./Icon.svelte";
  import { m } from "$lib/paraglide/messages";

  let {
    saved,
    busy = false,
    onsave,
    onbuy,
    move,
    close,
  }: {
    saved: boolean;
    busy?: boolean;
    onsave: () => void;
    onbuy: () => void;
    move?: { icon: IconName; title: string; onclick: () => void };
    close?: { title: string; disabled?: boolean; onclick: () => void };
  } = $props();
</script>

<div class="panel-head head">
  <span class="label">{m.items_item()}</span>
  <span class="acts">
    <button class="btn sm ghost" title={m.items_shared_title()} disabled={busy || saved} onclick={onsave}>
      <Icon name={saved ? "check" : "folder"} size={13} />
      {saved ? m.items_template_saved() : m.items_add_to_shared()}
    </button>
    <button class="btn sm ghost" title={m.items_buy_similar_title()} onclick={onbuy}>
      <Icon name="magnifying-glass" size={13} />
      {m.items_buy_similar()}
    </button>
    {#if move || close}<span class="vr" aria-hidden="true"></span>{/if}
    {#if move}
      <button class="btn sm ghost icon" title={move.title} aria-label={move.title} onclick={move.onclick}>
        <Icon name={move.icon} size={14} />
      </button>
    {/if}
    {#if close}
      <button class="btn sm ghost icon" title={close.title} aria-label={close.title} disabled={close.disabled} onclick={close.onclick}>
        <Icon name="x" size={14} />
      </button>
    {/if}
  </span>
</div>

<style>
  .head {
    gap: 8px;
    padding-right: 4px;
  }
  .acts {
    display: inline-flex;
    align-items: center;
    gap: 2px;
  }
  .icon {
    width: 24px;
    padding: 0;
    justify-content: center;
  }
  .vr {
    width: 1px;
    height: 14px;
    margin: 0 4px;
    background: var(--line-1);
  }
</style>
