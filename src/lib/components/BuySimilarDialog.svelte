<script lang="ts">
  import { scrollHint } from "$lib/scroll-hint";
  import { onMount } from "svelte";
  import { tradeWindow } from "$lib/state/trade.svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { engine, type BuySimilarInfo, type BuySimilarTarget } from "$lib/engine.svelte";
  import { game } from "$lib/state/game.svelte";
  import PobText from "./PobText.svelte";
  import { m } from "$lib/paraglide/messages";

  let { target, onclose }: { target: BuySimilarTarget; onclose: () => void } = $props();

  type Row = { checked: boolean; min: string; max: string };
  const KEY = "pob-redux:buy-similar";

  let info = $state<BuySimilarInfo | null>(null);
  let error = $state<string | null>(null);
  let note = $state<string | null>(null);
  let opening = $state(false);
  let leagues = $state<{ id: string; text: string }[]>([]);
  let realm = $state("");
  let league = $state("");
  let listed = $state(1);
  let baseType = $state(false);
  let ilvlMin = $state("");
  let ilvlMax = $state("");
  let defences = $state<Row[]>([]);
  let mods = $state<Row[]>([]);

  const prefsKey = () => `${KEY}:${game.current}`;

  function friendly(e: unknown) {
    return String(e).split("\n")[0].replace(/^EngineError: [a-z_]+: /, "").replace(/^runtime error: /, "");
  }

  const num = (s: string) => (s.trim() === "" || Number.isNaN(Number(s)) ? null : Number(s));

  onMount(async () => {
    let saved: { realm?: string; league?: string; listed?: number } = {};
    try {
      saved = JSON.parse(localStorage.getItem(prefsKey()) ?? "{}");
      league = saved.league ?? localStorage.getItem("pob-redux:trade-league") ?? "";
    } catch {}
    engine
      .tradeLeagues()
      .then((r) => {
        leagues = r.leagues;
        if (!r.leagues.some((l) => l.id === league) && r.leagues.length) league = r.leagues[0].id;
      })
      .catch(() => {
        leagues = [{ id: "Standard", text: "Standard" }];
        if (!league) league = "Standard";
      });
    try {
      const i = await engine.buySimilarInfo(target);
      realm = saved.realm && i.realms.includes(saved.realm) ? saved.realm : i.realms[0];
      listed = saved.listed && saved.listed >= 1 && saved.listed <= i.listed.length ? saved.listed : 1;
      defences = i.defences.map((d) => ({ checked: false, min: String(d.value), max: "" }));
      mods = i.mods.map((mod) => ({
        checked: false,
        min: mod.ranged && typeof mod.value === "number" && mod.value !== 0 ? String(mod.value) : "",
        max: "",
      }));
      info = i;
    } catch (e) {
      error = friendly(e);
    }
  });

  async function openSearch() {
    if (!info || opening) return;
    opening = true;
    error = null;
    note = null;
    try {
      const { url } = await engine.buySimilarUrl({
        ...target,
        realm,
        league,
        listed,
        baseType,
        ilvlMin: num(ilvlMin),
        ilvlMax: num(ilvlMax),
        defences: defences.map((r) => ({ checked: r.checked, min: num(r.min), max: num(r.max) })),
        mods: mods.map((r) => ({ checked: r.checked, min: num(r.min), max: num(r.max) })),
      });
      try {
        localStorage.setItem(prefsKey(), JSON.stringify({ realm, league, listed }));
      } catch {}
      await writeText(url).catch(() => {});
      await tradeWindow.open(url);
      note = m.buy_opened();
    } catch (e) {
      error = friendly(e);
    } finally {
      opening = false;
    }
  }
</script>

<div class="overlay" role="presentation" onclick={onclose}>
  <div
    class="modal"
    role="dialog"
    aria-modal="true"
    aria-label={m.buy_title()}
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.key === "Escape" && onclose()}
  >
    <div class="mhead">
      <span class="label">{m.buy_title()}</span>
      {#if info}<span class="iname" title={info.name}>{info.name}</span>{/if}
      <button class="btn sm ghost" onclick={onclose}>{m.common_close()}</button>
    </div>

    <div class="mbody scroll-hint" {@attach scrollHint}>
      {#if !info && !error}
        <div class="dim small">{m.buy_reading()}</div>
      {:else if info}
        <div class="row">
          {#if info.realms.length > 1}
            <label class="fld">
              <span class="label">{m.buy_realm()}</span>
              <select class="select sm" bind:value={realm}>
                {#each info.realms as r}<option value={r}>{r}</option>{/each}
              </select>
            </label>
          {/if}
          <label class="fld">
            <span class="label">{m.buy_league()}</span>
            <select class="select sm" bind:value={league} disabled={leagues.length === 0}>
              {#if leagues.length === 0}<option value={league}>{m.common_loading()}</option>{/if}
              {#each leagues as l (l.id)}<option value={l.id}>{l.text}</option>{/each}
            </select>
          </label>
          <label class="fld">
            <span class="label">{m.buy_listed()}</span>
            <select class="select sm" bind:value={listed}>
              {#each info.listed as l, i}<option value={i + 1}>{l}</option>{/each}
            </select>
          </label>
        </div>

        {#if info.unique}
          <div class="dim small">{m.buy_unique_note({ name: info.name })}</div>
        {:else}
          <div class="grid">
            <span class="dim small">{m.buy_category()}</span>
            <span class="small">{info.category}</span>
            <span></span>
            <span></span>
            {#if info.baseName}
              <label class="chk small span2"><input type="checkbox" bind:checked={baseType} /> {m.buy_only_base({ base: info.baseName })}</label>
              <span></span>
              <span></span>
            {/if}
            <span class="small">{m.buy_item_level()}</span>
            <span></span>
            <input class="input sm num" placeholder={m.common_min()} bind:value={ilvlMin} />
            <input class="input sm num" placeholder={m.common_max()} bind:value={ilvlMax} />
            {#each info.defences as d, i}
              <label class="chk small span2"><input type="checkbox" bind:checked={defences[i].checked} /> {d.label}</label>
              <input class="input sm num" placeholder={m.common_min()} bind:value={defences[i].min} />
              <input class="input sm num" placeholder={m.common_max()} bind:value={defences[i].max} />
            {/each}
          </div>
        {/if}

        <div class="grid mods">
          {#each info.mods as mod, i}
            <label class="chk small span2" class:off={!mod.searchable} title={mod.searchable ? "" : m.buy_mod_unsearchable()}>
              <input type="checkbox" bind:checked={mods[i].checked} disabled={!mod.searchable} />
              <span class="lines">{#each mod.lines as l}<span class="line"><PobText text={l} /></span>{/each}</span>
            </label>
            {#if mod.ranged && mod.searchable}
              <input class="input sm num" placeholder={m.common_min()} bind:value={mods[i].min} />
              <input class="input sm num" placeholder={m.common_max()} bind:value={mods[i].max} />
            {:else}
              <span></span>
              <span></span>
            {/if}
          {:else}
            <div class="dim small">{m.buy_no_mods()}</div>
          {/each}
        </div>
      {/if}
    </div>

    <div class="foot">
      {#if error}<span class="err small">{error}</span>{:else if note}<span class="dim small">{note}</span>{/if}
      <button class="btn sm primary" onclick={openSearch} disabled={!info || opening || !league}>{m.buy_open()}</button>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: var(--backdrop);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 110;
  }
  .modal {
    width: 660px;
    max-width: 92vw;
    max-height: 84vh;
    display: flex;
    flex-direction: column;
    background: var(--bg-1);
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    box-shadow: var(--shadow-modal);
  }
  .mhead {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border-bottom: 1px solid var(--line-0);
  }
  .mhead .btn {
    margin-left: auto;
  }
  .iname {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-sm);
    color: var(--fg-1);
  }
  .mbody {
    overflow-y: auto;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
  }
  .fld {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .grid {
    display: grid;
    grid-template-columns: 1fr auto 72px 72px;
    align-items: center;
    gap: 6px 8px;
  }
  .span2 {
    grid-column: span 2;
  }
  .chk {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    color: var(--fg-1);
  }
  .chk input {
    margin-top: 2px;
  }
  .chk.off {
    opacity: var(--fade-off);
  }
  .lines {
    display: flex;
    flex-direction: column;
  }
  .foot {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 10px;
    padding: 8px 12px 10px;
    border-top: 1px solid var(--line-0);
  }
  .foot .btn {
    margin-left: auto;
  }
  .err {
    color: var(--bad);
  }
</style>
