<script lang="ts">
  import ClassArt from "./ClassArt.svelte";
  import { build } from "$lib/state/build.svelte";
  import { m } from "$lib/paraglide/messages";

  let { mode, onclose }: { mode: "new" | "ascendancy"; onclose: () => void } = $props();

  const info = build.info;
  let classId = $state(info?.classId ?? 0);
  let ascId = $state(info?.ascendClassId ?? 0);
  let secondaryId = $state(info?.secondaryAscendClassId ?? 0);
  let level = $state(info?.level ?? 1);
  let auto = $state(info?.levelAuto ?? false);
  let busy = $state(false);

  const version = $derived(build.tree?.treeVersion ?? "");
  const cls = $derived(build.classes.find((c) => c.id === classId));
  const changed = $derived(
    !!info &&
      (classId !== info.classId || ascId !== info.ascendClassId || secondaryId !== (info.secondaryAscendClassId ?? 0) || auto !== info.levelAuto || (!auto && level !== info.level)),
  );

  const refund = $derived(mode === "ascendancy" && !!info?.ascendClassName && ascId !== info.ascendClassId && info.points.ascUsed > 0);

  function pickClass(id: number) {
    if (id === classId) return;
    classId = id;
    ascId = 0;
  }

  async function apply() {
    const now = build.info;
    if (!now || busy) return;
    busy = true;
    try {
      if (classId !== now.classId) await build.selectClass(classId, ascId);
      else if (ascId !== now.ascendClassId) await build.selectClass(undefined, ascId);
      if (secondaryId !== (now.secondaryAscendClassId ?? 0)) await build.selectClass(undefined, undefined, secondaryId);
      if (auto !== build.info?.levelAuto) await build.setLevelAuto(auto);
      if (!auto && level !== build.info?.level) await build.setLevel(level);
      onclose();
    } finally {
      busy = false;
    }
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} />

<div class="overlay" role="presentation" onclick={onclose}>
  <div class="modal" role="dialog" aria-modal="true" aria-label={mode === "new" ? m.char_new_title() : m.char_asc_title()} tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.key === "Enter" && changed && apply()}>
    <div class="mhead"><span class="label">{mode === "new" ? m.char_new_title() : m.char_asc_title()}</span></div>
    <div class="body">
      {#if mode === "new"}
        <span class="label">{m.char_class()}</span>
        <div class="picks">
          {#each build.classes as c (c.id)}
            <button class="pick" class:on={c.id === classId} onclick={() => pickClass(c.id)}>
              <ClassArt {version} className={c.name} size={56} />
              <span>{c.name}</span>
            </button>
          {/each}
        </div>
      {/if}
      <span class="label">{m.char_ascendancy()}</span>
      <div class="picks">
        <button class="pick" class:on={ascId === 0} onclick={() => (ascId = 0)}>
          <span class="none">—</span>
          <span>{m.sidebar_ascendancy_none()}</span>
        </button>
        {#each cls?.ascendancies ?? [] as a (a.id)}
          <button class="pick" class:on={a.id === ascId} onclick={() => (ascId = a.id)}>
            <ClassArt {version} className={cls?.name ?? ""} ascendancy={a.name} size={56} />
            <span>{a.name}</span>
          </button>
        {/each}
      </div>
      {#if build.secondaryAscendancies.length}
        <label class="row">
          <span class="label">{m.sidebar_second_ascendancy()}</span>
          <select class="select" bind:value={secondaryId}>
            <option value={0}>{m.sidebar_ascendancy_none()}</option>
            {#each build.secondaryAscendancies as a (a.id)}
              <option value={a.id}>{a.name}</option>
            {/each}
          </select>
        </label>
      {/if}
      {#if refund && info}
        <p class="note">{m.char_refund({ count: info.points.ascUsed, name: info.ascendClassName ?? "" })}</p>
      {/if}
    </div>
    <div class="foot">
      {#if mode === "new"}
        <span class="label">{m.sidebar_level()}</span>
        <input class="input num lvl" type="number" min="1" max="100" bind:value={level} disabled={auto} />
        <button class="btn sm ghost auto" class:on={auto} title={auto ? m.sidebar_level_auto_on() : m.sidebar_level_auto_off()} onclick={() => (auto = !auto)}>{m.sidebar_level_auto()}</button>
      {/if}
      <span class="grow"></span>
      <button class="btn sm ghost" onclick={onclose}>{m.common_cancel()}</button>
      {#if mode === "new"}
        <button class="btn sm primary" disabled={busy} onclick={() => (changed ? apply() : onclose())}>{m.char_start()}</button>
      {:else}
        <button class="btn sm primary" disabled={busy || !changed} onclick={apply}>{m.char_apply()}</button>
      {/if}
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 120;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--backdrop);
  }
  .modal {
    width: 680px;
    max-width: 92vw;
    max-height: 90vh;
    overflow-y: auto;
    background: var(--bg-1);
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    box-shadow: var(--shadow-modal);
  }
  .mhead {
    padding: 10px 14px;
    border-bottom: 1px solid var(--line-0);
  }
  .body {
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .picks {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .pick {
    appearance: none;
    width: 76px;
    padding: 6px 2px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
    border: 1px solid transparent;
    border-radius: var(--r-1);
    background: transparent;
    color: var(--fg-2);
    font: inherit;
    font-size: var(--fs-xs);
    text-align: center;
    cursor: var(--cursor-pointer, pointer);
  }
  .pick:hover {
    background: var(--bg-hover);
    color: var(--fg-0);
  }
  .pick.on {
    border-color: var(--line-2);
    background: var(--bg-active);
    color: var(--fg-0);
  }
  .pick :global(.art) {
    box-shadow: 0 0 0 1px var(--line-2);
  }
  .pick.on :global(.art) {
    box-shadow: 0 0 0 2px var(--fg-0);
  }
  .none {
    width: 56px;
    height: 56px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: var(--bg-3);
    color: var(--fg-3);
    box-shadow: 0 0 0 1px var(--line-2);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .row .select {
    width: 220px;
  }
  .foot {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 14px 10px;
    border-top: 1px solid var(--line-0);
  }
  .lvl {
    width: 56px;
    height: 24px;
    text-align: center;
  }
  .lvl::-webkit-inner-spin-button {
    display: none;
  }
  .auto {
    font-family: var(--font-mono);
    font-size: var(--fs-2xs);
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .auto.on {
    color: var(--ok);
    background: transparent;
  }
  .grow {
    flex: 1;
  }
  .note {
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--warn);
  }
</style>
