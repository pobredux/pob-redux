<script lang="ts">
  import { engine, type ItemEnchants, type ItemTarget, type ItemCustomizationEdit } from "$lib/engine.svelte";
  import { m } from "$lib/paraglide/messages";
  import SearchSelect from "./SearchSelect.svelte";

  let { target, revision, busy, onchange, onclose }: {
    target: ItemTarget;
    revision: string;
    busy: boolean;
    onchange: (edit: ItemCustomizationEdit) => unknown;
    onclose: () => void;
  } = $props();
  let error = $state<string | null>(null);

  let info = $state<ItemEnchants | null>(null);
  let skill = $state<string | null>(null);
  let source = $state<string | null>(null);
  let search = $state("");
  let slot = $state(1);

  $effect(() => {
    revision;
    const current = target;
    let active = true;
    const sk = skill;
    const src = source;
    engine
      .itemEnchants(current, sk ?? undefined, src ?? undefined)
      .then((r) => {
        if (!active) return;
        info = r;
        skill = r.skill;
        source = r.source;
      })
      .catch((e) => {
        if (active) error = String(e);
      });
    return () => { active = false; };
  });

  const lines = $derived.by(() => {
    const q = search.trim().toLowerCase();
    return (info?.lines ?? []).filter((l) => !q || l.toLowerCase().includes(q));
  });

  async function apply(line: string) {
    if (!busy) await onchange({ operation: "enchant", line, slot, skill: skill ?? undefined, source: source ?? undefined });
  }
  async function removeAt(i: number) {
    if (!busy) await onchange({ operation: "enchant", remove: true, slot: i + 1, skill: skill ?? undefined, source: source ?? undefined });
  }
</script>

<div class="modal" role="dialog" aria-modal="true" tabindex="-1">
  <div class="panel dialog enchdlg">
    <div class="label">{m.enchant_title()}</div>
    {#if error}<p class="err" role="alert">{error}</p>{/if}
    <fieldset disabled={busy}>
      {#if info && !info.available}
        <p class="dim small">{m.enchant_unavailable()}</p>
      {:else}
        <div class="filters">
          {#if info?.bySkill}
            <div class="skillpick">
              <span class="label">{m.enchant_skill()}</span>
              <div class="pick">
                <SearchSelect
                  value={skill ?? ""}
                  options={(info?.skills ?? []).map((s) => ({ value: s, label: s }))}
                  label={m.enchant_skill()}
                  onchange={(s) => (skill = s)}
                />
              </div>
            </div>
          {/if}
          <label class="fld-inline">
            <span class="label">{m.enchant_source()}</span>
            <select class="select sm" value={source ?? ""} onchange={(e) => (source = (e.target as HTMLSelectElement).value)}>
              {#each info?.sources ?? [] as s}
                <option value={s}>{s}</option>
              {/each}
            </select>
          </label>
          {#if (info?.slots ?? 1) > 1}
            <label class="fld-inline" title={m.enchant_slot_title()}>
              <span class="label">{m.enchant_slot()}</span>
              <select class="select sm" bind:value={slot}>
                {#each Array(info?.slots ?? 1) as _, i}
                  <option value={i + 1}>{i + 1}</option>
                {/each}
              </select>
            </label>
          {/if}
          <input class="input grow" placeholder={m.enchant_search()} bind:value={search} />
        </div>

        {#if info?.current.length}
          <div class="current">
            <span class="label">{m.enchant_current()}</span>
            {#each info.current as c, i (c + i)}
              <span class="pill">
                {c}
                <button class="mini x" title={m.common_remove()} onclick={() => removeAt(i)}>✕</button>
              </span>
            {/each}
          </div>
        {/if}

        <div class="scroll">
          {#each lines as l (l)}
            <button class="row" onclick={() => apply(l)}>{l}</button>
          {:else}
            <div class="dim small pad">{m.common_nothing_matches()}</div>
          {/each}
        </div>
      {/if}
    </fieldset>
    <div class="acts">
      <button class="btn ghost" onclick={onclose}>{m.common_close()}</button>
    </div>
  </div>
</div>

<style>
  .mini {
    appearance: none;
    border: 1px solid var(--line-1);
    background: var(--bg-2);
    color: var(--fg-2);
    font-size: var(--fs-2xs);
    height: 18px;
    padding: 0 6px;
    cursor: pointer;
    border-radius: 3px;
  }
  .mini:hover {
    color: var(--fg-0);
  }
  .mini.x:hover {
    color: var(--bad);
  }
  .small {
    font-size: var(--fs-xs);
  }
  .err {
    color: var(--bad);
  }

  fieldset {
    border: 0;
    padding: 0;
    margin: 0;
    min-width: 0;
    display: contents;
  }
  .modal {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    background: var(--backdrop);
    z-index: 6;
  }
  .enchdlg {
    width: 720px;
    max-height: 80vh;
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .filters {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
  }
  .skillpick {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .pick {
    width: 240px;
  }
  .filters .grow {
    flex: 1;
    min-width: 180px;
  }
  .current {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
  }
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 4px 2px 8px;
    font-size: var(--fs-xs);
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    color: var(--fg-1);
  }
  .scroll {
    overflow: auto;
    height: 44vh;
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
  }
  .row {
    display: block;
    width: 100%;
    padding: 5px 10px;
    background: none;
    border: 0;
    color: var(--fg-1);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .row:hover {
    background: var(--bg-hover);
    color: var(--fg-0);
  }
  .pad {
    padding: 8px 10px;
  }
  .acts {
    display: flex;
    justify-content: flex-end;
  }
</style>
