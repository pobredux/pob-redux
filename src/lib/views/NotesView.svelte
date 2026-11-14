<script lang="ts">
  import { tick } from "svelte";
  import { engine } from "$lib/engine.svelte";
  import { build } from "$lib/state/build.svelte";
  import PobText from "$lib/components/PobText.svelte";
  import { m } from "$lib/paraglide/messages";

  let text = $state("");
  let loadedFor = -1;
  let timer = 0;
  let area = $state<HTMLTextAreaElement | null>(null);
  let editing = $state(false);

  // PoB's own notes palette, from Data/Global.lua.
  const COLOURS: [string, string][] = [
    ["NORMAL", "^xC8C8C8"],
    ["MAGIC", "^x8888FF"],
    ["RARE", "^xFFFF77"],
    ["UNIQUE", "^xAF6025"],
    ["FIRE", "^xB97123"],
    ["COLD", "^x3F6DB3"],
    ["LIGHTNING", "^xADAA47"],
    ["CHAOS", "^xD02090"],
    ["STRENGTH", "^xE05030"],
    ["DEXTERITY", "^x70FF70"],
    ["INTELLIGENCE", "^x7070FF"],
    ["DEFAULT", "^7"],
  ];

  /** Drop a colour code at the cursor, the way PoB's buttons do. */
  function insert(code: string) {
    const el = area;
    if (!el) return;
    const at = el.selectionStart ?? text.length;
    const to = el.selectionEnd ?? at;
    text = text.slice(0, at) + code + text.slice(to);
    onInput();
    queueMicrotask(() => {
      el.focus();
      el.selectionStart = el.selectionEnd = at + code.length;
    });
  }

  $effect(() => {
    const key = build.info ? build.info.name + build.info.file : null;
    if (key === null) return;
    // The engine restarts rev on load, so a drop means another build was opened.
    const rev = build.rev;
    if (loadedFor === -1 || rev < loadedFor) {
      engine.getNotes().then((r) => {
        text = r.text;
        editing = !r.text.trim();
      });
    }
    loadedFor = rev;
  });

  async function toggleEdit() {
    editing = !editing;
    if (editing) {
      await tick();
      area?.focus();
    }
  }

  function onInput() {
    clearTimeout(timer);
    timer = window.setTimeout(() => build.run(() => engine.setNotes(text), { sync: false }), 400);
  }
</script>

<div class="page">
  <div class="toolbar">
    {#each COLOURS as [name, code] (name)}
      <button class="swatch" disabled={!editing} title={m.notes_insert({ name, code })} onclick={() => insert(code)}>
        <PobText text={code + name} />
      </button>
    {/each}
    <span class="vr"></span>
    <button class="btn sm ghost" class:on={editing} aria-pressed={editing} title={m.notes_edit_title()} onclick={toggleEdit}>{m.notes_edit()}</button>
  </div>
  {#if editing}
    <textarea class="notes selectable" bind:this={area} bind:value={text} oninput={onInput} placeholder={m.notes_placeholder()}></textarea>
  {:else}
    <div class="notes preview selectable">
      {#if text.trim()}<PobText text={text} />{:else}<span class="dim">{m.notes_empty()}</span>{/if}
    </div>
  {/if}
</div>

<style>
  .page {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .toolbar {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 4px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--line-0);
    background: var(--bg-1);
  }
  .swatch {
    appearance: none;
    background: none;
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    padding: 2px 7px;
    font-size: var(--fs-xs);
    letter-spacing: 0.04em;
    cursor: var(--cursor-pointer, pointer);
  }
  .swatch:hover:not(:disabled) {
    background: var(--bg-hover);
  }
  .swatch:disabled {
    opacity: var(--fade-off);
    cursor: var(--cursor-arrow, default);
  }
  .preview {
    overflow: auto;
    white-space: pre-wrap;
  }
  .notes {
    flex: 1;
    border: 0;
    outline: none;
    resize: none;
    background: var(--bg-0);
    color: var(--fg-0);
    padding: 16px 20px;
    font-family: var(--font-mono);
    font-size: var(--fs-sm);
    line-height: 1.55;
  }
</style>
