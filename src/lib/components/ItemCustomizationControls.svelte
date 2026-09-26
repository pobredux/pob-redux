<script lang="ts">
  import { engine, type ItemCustomization, type ItemCustomizationEdit, type ItemTarget } from "$lib/engine.svelte";
  import ItemAdvancedControls from "./ItemAdvancedControls.svelte";
  import ItemAffixSelector from "./ItemAffixSelector.svelte";
  import { m } from "$lib/paraglide/messages";

  let { data, target, busy = false, sourceSlot, onchange, onpendingchange }: {
    data: ItemCustomization;
    target: ItemTarget;
    busy?: boolean;
    sourceSlot?: string;
    onchange: (edit: ItemCustomizationEdit) => Promise<ItemCustomization | false | undefined>;
    onpendingchange: (pending: boolean) => void;
  } = $props();

  let changingAffix = $state(false);
  const controlsBusy = $derived(busy || changingAffix);

  function beginAffixChange(): boolean {
    if (controlsBusy) return false;
    changingAffix = true;
    onpendingchange(true);
    return true;
  }

  function endAffixChange() {
    changingAffix = false;
    onpendingchange(false);
  }

  let source = $state<"Custom" | "Prefix" | "Suffix">("Custom");
  let query = $state("");
  let customText = $state("");
  let options = $state<{ id: string; label: string; level: number }[]>([]);
  let total = $state(0);
  let option = $state("");
  let loading = $state(false);
  let error = $state<string | null>(null);
  const modifierLabels: Record<string, string> = $derived({
    implicit: m.items_modifier_implicit(),
    explicit: m.items_modifier_explicit(),
    enchant: m.items_modifier_enchant(),
  });

  $effect(() => {
    if (data.affixes.crafted) source = "Custom";
  });

  $effect(() => {
    const currentTarget = target;
    const currentSource = source;
    const currentQuery = query;
    let active = true;
    options = [];
    option = "";
    error = null;
    loading = currentSource !== "Custom";
    const timer = setTimeout(async () => {
      if (currentSource === "Custom") return;
      try {
        const result = await engine.itemModifierOptions(currentTarget, currentSource, currentQuery);
        if (active) {
          options = result.options;
          total = result.total;
          option = result.options[0]?.id ?? "";
        }
      } catch (e) {
        if (active) error = String(e);
      } finally {
        if (active) loading = false;
      }
    }, 180);
    return () => { active = false; clearTimeout(timer); };
  });

  function changeVariant(index: number, value: number) {
    onchange({ operation: "variant", picks: data.variants.picks.map((p, i) => i === index ? value : p) });
  }
</script>

<fieldset disabled={controlsBusy} class="controls">
  <legend class="label">{m.items_customize()}</legend>
  <div class="row">
    {#if data.canQuality}
      <label>{m.items_quality()} <input class="input num" type="number" min="0" max="50" value={data.quality} onchange={(e) => onchange({ operation: "props", quality: Number(e.currentTarget.value) })} /></label>
    {/if}
    <label>{m.items_item_level()} <input class="input num" type="number" min="1" max="100" value={data.itemLevel} onchange={(e) => onchange({ operation: "props", itemLevel: Number(e.currentTarget.value) })} /></label>
    <label><input type="checkbox" checked={data.corrupted} onchange={(e) => onchange({ operation: "props", corrupted: e.currentTarget.checked })} /> {m.items_custom_corrupted()}</label>
  </div>
  <div class="row">
    {#if data.canQuality}<button class="btn sm" onclick={() => onchange({ operation: "normalize" })}>{m.items_normalize_quality()}</button>{/if}
    {#if target.raw !== undefined}
      {#if data.canCopyAnoints}<button class="btn sm" onclick={() => onchange({ operation: "copy_anoints", sourceSlot })}>{m.items_copy_anoints()}</button>{/if}
      {#if data.canCopyAugments}<button class="btn sm" onclick={() => onchange({ operation: "copy_augments", sourceSlot })}>{m.items_copy_augments()}</button>{/if}
    {/if}
  </div>

  {#if data.runeSocketLimit > 0}
    <label>{m.items_rune_sockets()} <input class="input num" type="number" min="0" max={data.runeSocketLimit} value={data.runes.socketCount} onchange={(e) => onchange({ operation: "rune_sockets", count: Number(e.currentTarget.value) })} /></label>
  {/if}
  {#if data.runes.socketCount > 0}
    <div class="label">{m.items_runes()}</div>
    {#each data.runes.runes as rune, index}
      <select class="select" aria-label={m.items_rune_number({ index: index + 1 })} value={rune} onchange={(e) => onchange({ operation: "rune", index: index + 1, name: e.currentTarget.value })}>
        {#each data.runes.options as opt (opt.name)}
          <option value={opt.name}>{opt.name === "None" ? m.items_empty_socket() : `${opt.name} · ${opt.label ?? opt.lines[0] ?? ""}`}</option>
        {/each}
      </select>
    {/each}
  {/if}

  {#if data.variants.names.length > 1}
    <div class="label">{m.items_variants()}</div>
    {#each data.variants.picks as pick, index}
      <select class="select" aria-label={m.items_variant_number({ index: index + 1 })} value={pick} onchange={(e) => changeVariant(index, Number(e.currentTarget.value))}>
        {#each data.variants.names as name, i}<option value={i + 1}>{name}</option>{/each}
      </select>
    {/each}
  {/if}

  {#if data.affixes.crafted}
    {#each ["prefixes", "suffixes"] as table}
      <div class="label">{table === "prefixes" ? m.items_prefixes() : m.items_suffixes()}</div>
      {#each (table === "prefixes" ? data.affixes.prefixes : data.affixes.suffixes) as slot (slot.index)}
        <ItemAffixSelector {slot} table={table as "prefixes" | "suffixes"} {target} {onchange}
          onbegin={beginAffixChange} onend={endAffixChange} />
      {/each}
    {/each}
  {/if}

  {#if data.catalyst.usable}
    <label>{m.items_catalyst()}
      <select class="select" value={data.catalyst.catalyst} onchange={(e) => onchange({ operation: "props", catalyst: Number(e.currentTarget.value) })}>
        <option value="0">{m.items_catalyst_none()}</option>
        {#each data.catalyst.names as name, i}<option value={i + 1}>{name}</option>{/each}
      </select>
    </label>
    {#if data.catalyst.catalyst > 0}
      <label>{m.items_catalyst_quality_title()} <input class="input num" type="number" min="0" max="100" value={data.catalyst.quality} onchange={(e) => onchange({ operation: "props", catalystQuality: Number(e.currentTarget.value) })} /></label>
    {/if}
  {/if}

  <details>
    <summary class="label">{m.items_modifiers()}</summary>
    <div class="modifiers">
      {#if data.affixes.crafted}<p class="dim">{m.items_crafted_modifiers_hint()}</p>{/if}
      {#each data.modifiers as mod (`${mod.section}:${mod.index}`)}
        <div class="modifier">
          <div class="row">
            <label><input type="checkbox" checked={!mod.disabled} aria-label={m.items_enable_modifier({ text: mod.text })} onchange={(e) => onchange({ operation: "modifier", section: mod.section, index: mod.index, disabled: !e.currentTarget.checked })} /> {modifierLabels[mod.section]}</label>
            {#if !mod.parsed}<span class="dim">{m.items_mod_unrecognized()}</span>{/if}
            <button class="btn sm ghost" onclick={() => onchange({ operation: "modifier", section: mod.section, index: mod.index, remove: true })}>{m.common_delete()}</button>
          </div>
          <input class="input modtext" aria-label={m.items_modifier_text()} value={mod.text} onchange={(e) => onchange({ operation: "modifier", section: mod.section, index: mod.index, text: e.currentTarget.value })} />
          {#if mod.range != null}
            <label class="roll">{m.items_roll_percent({ value: Math.round(mod.range * 100) })}
              <input type="range" min="0" max="100" value={Math.round(mod.range * 100)} onchange={(e) => onchange({ operation: "modifier", section: mod.section, index: mod.index, range: Number(e.currentTarget.value) / 100 })} />
            </label>
          {/if}
        </div>
      {/each}
      <div class="label">{m.items_add_modifier()}</div>
      <select class="select" aria-label={m.items_modifier_source()} bind:value={source}>
        <option value="Custom">{m.items_custom_modifier()}</option>
        {#if !data.affixes.crafted}
          <option value="Prefix">{m.items_prefixes()}</option>
          <option value="Suffix">{m.items_suffixes()}</option>
        {/if}
      </select>
      {#if source === "Custom"}
        <input class="input" aria-label={m.items_new_modifier()} placeholder={m.items_modifier_example()} bind:value={customText} />
      {:else}
        <input class="input" aria-label={m.items_search_modifiers()} placeholder={m.common_search()} bind:value={query} />
        <select class="select" aria-label={m.items_new_modifier()} bind:value={option} disabled={loading}>
          {#each options as opt (opt.id)}<option value={opt.id}>{opt.label} · {m.items_mod_level({ level: opt.level })}</option>{/each}
        </select>
        <span class="dim">{loading ? m.items_mod_loading() : m.items_mod_count({ shown: options.length, total })}</span>
      {/if}
      {#if error}<div class="error" role="alert">{error}</div>{/if}
      <button class="btn sm" disabled={source === "Custom" ? !customText.trim() : loading || !option} onclick={() => onchange(source === "Custom" ? { operation: "add_modifier", text: customText } : { operation: "add_modifier", modId: option })}>{m.items_add_modifier()}</button>
    </div>
  </details>
</fieldset>

{#key target.itemId ?? "draft"}
  <ItemAdvancedControls {data} {target} busy={controlsBusy} {onchange} />
{/key}

<style>
  .controls { border: 0; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 8px; min-width: 0; font-size: var(--fs-xs); }
  legend { margin-bottom: 8px; }
  .row { display: flex; gap: 8px; flex-wrap: wrap; align-items: center; }
  label { display: flex; gap: 6px; align-items: center; flex-wrap: wrap; }
  .num { width: 70px; }
  .select, .modtext { width: 100%; min-width: 0; font-size: var(--fs-xs); }
  .roll { color: var(--fg-2); }
  .roll input { flex: 1; min-width: 80px; accent-color: var(--fg-1); }
  summary { cursor: pointer; padding: 6px 0; }
  .modifiers { display: flex; flex-direction: column; gap: 8px; }
  .modifier { border-top: 1px solid var(--line-0); padding-top: 8px; display: flex; flex-direction: column; gap: 6px; }
  .error { color: var(--bad); }
</style>
