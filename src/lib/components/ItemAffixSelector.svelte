<script lang="ts">
  import { untrack } from "svelte";
  import {
    engine,
    type AffixRollStep,
    type AffixRollTier,
    type AffixSlot,
    type ItemCustomization,
    type ItemCustomizationEdit,
    type ItemTarget,
  } from "$lib/engine.svelte";
  import { m } from "$lib/paraglide/messages";

  let { slot, table, target, onchange, onbegin, onend }: {
    slot: AffixSlot;
    table: "prefixes" | "suffixes";
    target: ItemTarget;
    onchange: (edit: ItemCustomizationEdit) => Promise<ItemCustomization | false | undefined>;
    onbegin: () => boolean;
    onend: () => void;
  } = $props();

  type Choice = { modId: string; affix: string | null; tier: number; step: AffixRollStep; position: number };
  const segment = 101;

  const families = $derived(slot.options);
  const selectedFamily = $derived(families.find((series) => series.modIds.includes(slot.modId)));
  const missingCurrent = $derived(slot.modId !== "None" && !selectedFamily);
  const selectedSeries = $derived(selectedFamily?.id ?? (missingCurrent ? slot.modId : ""));
  const seriesMods = $derived(selectedFamily?.modIds.join("|") ?? "");
  const missingLabel = $derived(slot.affix
    ? `${slot.affix}: ${slot.value ?? slot.label ?? slot.modId}`
    : slot.value ?? slot.label ?? slot.modId);
  const slotIndex = $derived(slot.index);
  const savedRoll = $derived(`${slot.modId}|${slot.range}|${slot.rangeIsTable}|${slot.value}`);
  let tiers = $state<AffixRollTier[]>([]);
  let loading = $state(false);
  let changing = $state(false);
  let error = $state<string | null>(null);
  let localChoice = $state<Choice | null>(null);
  let submitted: string | null = null;
  let loadedSeries = "";
  let pendingRolls: { seriesId: string; tiers: AffixRollTier[] | null } | null = null;
  let rollRefresh = $state(0);

  $effect(() => {
    rollRefresh;
    const seriesId = selectedSeries;
    const modIds = seriesMods;
    const index = slotIndex;
    const currentTarget = untrack(() => target);
    const currentRolls = untrack(() => slot.rolls);
    let active = true;
    if (pendingRolls?.seriesId === seriesId) {
      if (pendingRolls.tiers) {
        tiers = pendingRolls.tiers;
        loadedSeries = seriesId;
        pendingRolls = null;
        loading = false;
      } else {
        loading = true;
      }
      return;
    }
    if (currentRolls?.seriesId === seriesId && modIds) {
      tiers = currentRolls.tiers;
      loadedSeries = seriesId;
      loading = false;
      return;
    }
    if (loadedSeries !== seriesId || !modIds) tiers = [];
    error = null;
    loading = !!seriesId && !!modIds;
    if (seriesId && modIds) {
      engine.itemAffixRolls(currentTarget, table, index, seriesId).then(
        (result) => { if (active) { tiers = result.tiers; loadedSeries = seriesId; } },
        (e) => { if (active) error = String(e); },
      ).finally(() => { if (active) loading = false; });
    }
    return () => { active = false; };
  });

  $effect(() => {
    savedRoll;
    localChoice = null;
    submitted = null;
  });

  function restoreFocusAfterBusy(input: HTMLInputElement) {
    const fieldset = input.closest("fieldset");
    let observer: MutationObserver | null = null;
    function onFocusOut(event: FocusEvent) {
      if (event.target !== input) return;
      if (!fieldset || !input.matches(":disabled")) return;
      observer?.disconnect();
      observer = new MutationObserver(() => {
        if (!input.isConnected || input.matches(":disabled")) return;
        observer?.disconnect();
        observer = null;
        if (document.activeElement === document.body) input.focus({ preventScroll: true });
      });
      observer.observe(fieldset, { attributes: true, attributeFilter: ["disabled"] });
    }
    document.addEventListener("focusout", onFocusOut, true);
    return {
      destroy() {
        document.removeEventListener("focusout", onFocusOut, true);
        observer?.disconnect();
      },
    };
  }

  function sliderPosition(tierIndex: number, stepPosition: number): number {
    return tierIndex * segment + stepPosition;
  }

  function makeChoice(tierIndex: number, step: AffixRollStep): Choice {
    const tier = tiers[tierIndex];
    return { modId: tier.modId, affix: tier.affix, tier: tier.tier, step, position: sliderPosition(tierIndex, step.position) };
  }

  function nearest(position: number): Choice | null {
    if (!tiers.length) return null;
    const tierIndex = Math.min(tiers.length - 1, Math.max(0, Math.floor(position / segment)));
    const tier = tiers[tierIndex];
    const inTier = Math.min(100, Math.max(0, position - tierIndex * segment));
    const step = tier.steps.reduce((best, candidate) =>
      Math.abs(candidate.position - inTier) < Math.abs(best.position - inTier) ? candidate : best,
    );
    return makeChoice(tierIndex, step);
  }

  const savedChoice: Choice | null = $derived.by(() => {
    const tierIndex = tiers.findIndex((tier) => tier.modId === slot.modId);
    if (tierIndex < 0) return null;
    const tier = tiers[tierIndex];
    const range = slot.range ?? 0.5;
    const matchingStep = slot.rangeIsTable ? undefined : tier.steps.find((candidate) => candidate.value === slot.value);
    const step = matchingStep ?? tier.steps.reduce((best, candidate) =>
      Math.abs(candidate.range - range) < Math.abs(best.range - range) ? candidate : best,
    );
    return makeChoice(tierIndex, step);
  });
  const shownChoice = $derived(localChoice ?? savedChoice);
  const valueText = $derived(localChoice?.step.value ?? slot.value ?? shownChoice?.step.value ?? slot.label ?? "");
  const choices: Choice[] = $derived(tiers.flatMap((tier, tierIndex) => tier.steps.map((step) => makeChoice(tierIndex, step))));

  async function changeFamily(seriesId: string) {
    if (changing || seriesId === selectedSeries || !onbegin()) return;
    const fraction = shownChoice && tiers.length ? shownChoice.position / (tiers.length * segment - 1) : (slot.range ?? 0.5);
    changing = true;
    error = null;
    try {
      if (!seriesId) {
        await onchange({ operation: "affix", table, index: slot.index, modId: "None" });
        return;
      }
      pendingRolls = { seriesId, tiers: null };
      const updated = await onchange({ operation: "affix", table, index: slot.index, seriesId, relativePosition: fraction });
      const selected = updated && updated.affixes[table][slot.index - 1]?.rolls;
      if (selected && selected.seriesId === seriesId) {
        if (pendingRolls?.seriesId === seriesId) pendingRolls.tiers = selected.tiers;
        if (selectedSeries === seriesId) {
          tiers = selected.tiers;
          loadedSeries = seriesId;
          pendingRolls = null;
          loading = false;
        }
      } else {
        pendingRolls = null;
        if (selectedSeries === seriesId) rollRefresh++;
      }
    } catch (e) {
      pendingRolls = null;
      if (selectedSeries === seriesId) rollRefresh++;
      error = String(e);
    } finally {
      changing = false;
      onend();
    }
  }

  function choose(rawPosition: number, input: HTMLInputElement): Choice | null {
    const choice = nearest(rawPosition);
    if (choice) {
      if (localChoice?.position !== choice.position) submitted = null;
      localChoice = choice;
      input.value = String(choice.position);
    }
    return choice;
  }

  function keydown(event: KeyboardEvent & { currentTarget: EventTarget & HTMLInputElement }) {
    const direction = event.key === "ArrowRight" || event.key === "ArrowUp" ? 1
      : event.key === "ArrowLeft" || event.key === "ArrowDown" ? -1 : 0;
    if (!direction && event.key !== "Home" && event.key !== "End") return;
    event.preventDefault();
    if (!choices.length) return;
    const current = shownChoice;
    const index = current ? choices.findIndex((choice) => choice.modId === current.modId && choice.position === current.position) : -1;
    const nextIndex = event.key === "Home" ? 0 : event.key === "End" ? choices.length - 1
      : Math.max(0, Math.min(choices.length - 1, index + direction));
    choose(choices[nextIndex].position, event.currentTarget);
  }

  function keyup(event: KeyboardEvent & { currentTarget: EventTarget & HTMLInputElement }) {
    if (!["ArrowRight", "ArrowLeft", "ArrowUp", "ArrowDown", "Home", "End"].includes(event.key)) return;
    commit(Number(event.currentTarget.value), event.currentTarget);
  }

  async function commit(rawPosition: number, input: HTMLInputElement) {
    const choice = localChoice ?? choose(rawPosition, input);
    if (!choice) return;
    if (choice.modId === savedChoice?.modId && choice.step === savedChoice.step) {
      localChoice = null;
      return;
    }
    const key = `${choice.modId}:${choice.step.range}`;
    if (submitted === key) return;
    if (choice.modId !== slot.modId || Math.abs(choice.step.range - (slot.range ?? 0.5)) > 0.00001) {
      if (!onbegin()) {
        localChoice = null;
        return;
      }
      submitted = key;
      try {
        const result = await onchange({ operation: "affix", table, index: slot.index, modId: choice.modId, range: choice.step.range });
        if (result !== undefined && result !== false) return;
      } catch (e) {
        error = String(e);
      } finally {
        onend();
      }
      localChoice = null;
      submitted = null;
    }
  }
</script>

<select class="select" aria-label={table === "prefixes" ? m.items_prefix_number({ index: slot.index }) : m.items_suffix_number({ index: slot.index })}
  value={selectedSeries} disabled={changing} onchange={(e) => {
    const seriesId = e.currentTarget.value;
    e.currentTarget.value = selectedSeries;
    void changeFamily(seriesId);
  }}>
  <option value="">{table === "prefixes" ? m.items_empty_prefix() : m.items_empty_suffix()}</option>
  {#if missingCurrent}<option value={slot.modId}>{missingLabel}</option>{/if}
  {#each families as family (family.id)}<option value={family.id}>{family.label}</option>{/each}
</select>
{#if slot.modId !== "None"}
  <div class="affix-roll">
    <div class="affix-detail">
      <span>{shownChoice ? [shownChoice.affix, m.items_affix_tier({ tier: shownChoice.tier })].filter(Boolean).join(" - ") : slot.affix}</span>
      <span class="value">{valueText}</span>
    </div>
    {#if choices.length > 1 && shownChoice}
      <div class="slider-wrap">
        <input use:restoreFocusAfterBusy type="range" min="0" max={tiers.length * segment - 1} step="1" value={shownChoice.position} disabled={changing}
          aria-label={m.items_affix_roll()}
          aria-valuetext={`${valueText}, ${shownChoice.affix ?? ""}, ${m.items_affix_tier({ tier: shownChoice.tier })}`}
          oninput={(e) => choose(Number(e.currentTarget.value), e.currentTarget)}
          onpointerup={(e) => commit(Number(e.currentTarget.value), e.currentTarget)}
          onkeydown={keydown}
          onkeyup={keyup}
          onchange={(e) => commit(Number(e.currentTarget.value), e.currentTarget)} />
        {#each tiers.slice(1) as _, index}
          <span class="tick" style:left={`calc(8px + (100% - 16px) * ${(index + 1) / tiers.length})`} aria-hidden="true"></span>
        {/each}
      </div>
    {:else if loading}
      <span class="dim">{m.items_mod_loading()}</span>
    {/if}
  </div>
{/if}
{#if error}<span class="error" role="alert">{error}</span>{/if}

<style>
  .select { width: 100%; min-width: 0; font-size: var(--fs-xs); }
  .affix-roll { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  .affix-detail { display: flex; flex-wrap: wrap; gap: 4px 10px; color: var(--fg-2); }
  .value { color: var(--fg-1); }
  .slider-wrap { position: relative; min-width: 100px; }
  input[type="range"] { width: 100%; margin: 0; position: relative; z-index: 1; accent-color: var(--fg-1); }
  .tick { position: absolute; z-index: 2; top: 2px; bottom: 2px; width: 2px; background: var(--fg-2); pointer-events: none; }
  .error { color: var(--bad); }
</style>
