<script lang="ts">
  import { untrack } from "svelte";
  import { engine, type MinionLibrary, type MinionEntry } from "$lib/engine.svelte";
  import { build } from "$lib/state/build.svelte";
  import { m } from "$lib/paraglide/messages";

  let { onclose, kind: initialKind = "spectre" }: { onclose: () => void; kind?: "spectre" | "beast" } = $props();

  // The dialog is mounted fresh each time it opens, so the caller's choice is
  // only a starting tab; switching inside it must stick.
  let kind = $state<"spectre" | "beast">(untrack(() => initialKind));
  let lib = $state<MinionLibrary | null>(null);
  /** Edited here and only written back on Save, as PoB's popup does. */
  let owned = $state<MinionEntry[]>([]);
  let search = $state("");
  let category = $state("");
  let recommendedOnly = $state(false);
  let saving = $state(false);

  $effect(() => {
    const k = kind;
    engine
      .minionLibrary(k)
      .then((r) => {
        lib = r;
        owned = [...r.owned];
      })
      .catch((e) => (build.error = String(e)));
  });

  const ownedIds = $derived(new Set(owned.map((e) => e.id)));
  const shown = $derived.by(() => {
    const q = search.trim().toLowerCase();
    return (lib?.available ?? []).filter(
      (e) =>
        !ownedIds.has(e.id) &&
        (!q || e.name.toLowerCase().includes(q)) &&
        (!category || e.category === category) &&
        (!recommendedOnly || e.recommended),
    );
  });

  function add(e: MinionEntry) {
    if (!ownedIds.has(e.id)) owned = [...owned, e];
  }
  function remove(id: string) {
    owned = owned.filter((e) => e.id !== id);
  }

  async function save() {
    saving = true;
    try {
      await build.run(() => engine.setMinionLibrary(kind, owned.map((e) => e.id)));
      onclose();
    } catch (e) {
      build.error = String(e);
    } finally {
      saving = false;
    }
  }
</script>

<div class="modal">
  <div class="panel dialog libdlg">
    <div class="head">
      <span class="label">{kind === "beast" ? m.minions_beast_library() : m.minions_spectre_library()}</span>
      {#if lib?.hasBeasts}
        <span class="seg">
          <button class:on={kind === "spectre"} onclick={() => (kind = "spectre")}>{m.minions_spectres()}</button>
          <button class:on={kind === "beast"} onclick={() => (kind = "beast")}>{m.minions_beasts()}</button>
        </span>
      {/if}
    </div>

    <div class="filters">
      <input class="input grow" placeholder={m.minions_search()} bind:value={search} />
      {#if (lib?.categories.length ?? 0) > 0}
        <select class="select sm" bind:value={category}>
          <option value="">{m.common_all_types()}</option>
          {#each lib?.categories ?? [] as c}
            <option value={c}>{c}</option>
          {/each}
        </select>
      {/if}
      <label class="chk small" title={m.minions_recommended_title()}>
        <input type="checkbox" bind:checked={recommendedOnly} />
        {m.minions_recommended()}
      </label>
    </div>

    <div class="cols">
      <section class="side">
        <div class="shead">{m.minions_available()} <span class="dim num">{shown.length}</span></div>
        <div class="scroll">
          {#each shown as e (e.id)}
            <button class="row" onclick={() => add(e)} title={e.id}>
              <span class="nm">{e.name}</span>
              {#if e.recommended}<span class="tag">★</span>{/if}
              {#if e.category}<span class="dim small">{e.category}</span>{/if}
            </button>
          {:else}
            <div class="dim small pad">{m.common_nothing_matches()}</div>
          {/each}
        </div>
      </section>
      <section class="side">
        <div class="shead">{m.minions_owned()} <span class="dim num">{owned.length}</span></div>
        <div class="scroll">
          {#each owned as e (e.id)}
            <button class="row own" onclick={() => remove(e.id)} title={m.minions_remove()}>
              <span class="nm">{e.name}</span>
              <span class="x">✕</span>
            </button>
          {:else}
            <div class="dim small pad">{m.minions_owned_empty()}</div>
          {/each}
        </div>
      </section>
    </div>

    <p class="note dim small">{kind === "beast" ? m.minions_note_beast() : m.minions_note_spectre()}</p>

    <div class="acts">
      <button class="btn primary" disabled={saving || build.busy > 0} onclick={save}>{m.common_save()}</button>
      <button class="btn ghost" onclick={onclose}>{m.common_cancel()}</button>
    </div>
  </div>
</div>

<style>
  .modal {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: var(--backdrop);
    z-index: 6;
  }
  .libdlg {
    width: 760px;
    max-height: 80vh;
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }
  .seg {
    display: inline-flex;
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    overflow: hidden;
  }
  .seg button {
    padding: 3px 10px;
    font-size: var(--fs-xs);
    color: var(--fg-2);
    background: transparent;
    border: 0;
    cursor: var(--cursor-pointer, pointer);
  }
  .seg button.on {
    background: var(--bg-active);
    color: var(--fg-0);
  }
  .filters {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .filters .grow {
    flex: 1;
  }
  .cols {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
    min-height: 0;
  }
  .side {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
  }
  .shead {
    padding: 6px 10px;
    font-size: var(--fs-xs);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--fg-2);
    border-bottom: 1px solid var(--line-0);
  }
  .scroll {
    overflow: auto;
    height: 46vh;
  }
  .row {
    display: flex;
    align-items: baseline;
    gap: 8px;
    width: 100%;
    padding: 4px 10px;
    background: none;
    border: 0;
    color: var(--fg-1);
    font: inherit;
    text-align: left;
    cursor: var(--cursor-pointer, pointer);
  }
  .row:hover {
    background: var(--bg-hover);
    color: var(--fg-0);
  }
  .nm {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tag {
    color: var(--c-spirit);
  }
  .own .x {
    color: var(--fg-3);
  }
  .own:hover .x {
    color: var(--bad);
  }
  .pad {
    padding: 8px 10px;
  }
  .note {
    margin: 0;
  }
  .acts {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
