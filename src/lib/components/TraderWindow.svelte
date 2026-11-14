<script lang="ts">
  import { onMount } from "svelte";
  import { tradeWindow } from "$lib/state/trade.svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import {
    engine,
    gggTradeRequest,
    type ItemSetInfo,
    type PowerStat,
    type Tooltip,
    type TradeOptions,
    type TradeQueryOptions,
    type TradeResult,
    type TradeRow,
    type TradeSort,
    type TradeTarget,
    type TradeWeight,
  } from "$lib/engine.svelte";
  import { build } from "$lib/state/build.svelte";
  import { game } from "$lib/state/game.svelte";
  import { ggg } from "$lib/state/ggg.svelte";
  import PobTooltip from "$lib/components/PobTooltip.svelte";
  import SearchSelect from "$lib/components/SearchSelect.svelte";
  import { m } from "$lib/paraglide/messages";

  let { onclose, focusSlot = null }: { onclose: () => void; focusSlot?: string | null } = $props();

  interface Filter {
    tradeId: string;
    value: string;
  }

  const FILTER_KINDS = ["required", "blocked"] as const;
  type FilterKind = (typeof FILTER_KINDS)[number];
  const MAX_FILTERS = 5;

  interface Row extends TradeRow {
    url: string;
    error: string | null;
    /** Why a row with a search link has no listings. */
    hint: string | null;
    results: TradeResult[] | null;
    picked: number;
    equipSlot: string | null;
    nodeId: number | null;
    ringSlot: string | null;
    allMods: boolean;
    filtersOpen: boolean;
    required: Filter[];
    blocked: Filter[];
  }

  const rarityColor: Record<string, string> = {
    UNIQUE: "var(--c-unique)",
    RARE: "var(--c-rare)",
    MAGIC: "var(--c-magic)",
    NORMAL: "var(--c-normal)",
    RELIC: "var(--c-gem)",
  };

  const KEY = "pob-redux:trader";

  interface Saved {
    league: string;
    realm: string;
    status: string;
    sort: TradeSort;
    pages: number;
    weights: TradeWeight[];
    corrupted: boolean;
    mirrored: boolean;
    maxPrice: number;
    maxPriceType: string;
    maxLevel: number;
    sockets: number;
    links: number;
    jewelType: string;
    runeBehaviour: string;
    anointBehaviour: string;
    eldritch: string;
    enchantMode: string;
    scourge: boolean;
    talisman: boolean;
  }

  // The defaults are the ones Path of Building's Trader opens with.
  function load(): Saved {
    const fallback: Saved = {
      league: "Standard",
      realm: "pc",
      status: "securable",
      sort: "StatValue",
      pages: 2,
      weights: [
        { stat: "FullDPS", weightMult: 1 },
        { stat: "TotalEHP", weightMult: 0.5 },
      ],
      corrupted: true,
      mirrored: true,
      maxPrice: 0,
      maxPriceType: "",
      maxLevel: 0,
      sockets: 0,
      links: 0,
      jewelType: "Base",
      runeBehaviour: "Copy Current",
      anointBehaviour: "Copy Current",
      eldritch: "Copy Current",
      enchantMode: "Keep",
      scourge: true,
      talisman: false,
    };
    try {
      const raw = localStorage.getItem(KEY);
      if (raw) {
        const old = JSON.parse(raw) as Partial<Saved> & { runes?: boolean };
        if (old.runeBehaviour === undefined && old.runes === true) old.runeBehaviour = "Keep";
        return { ...fallback, ...old };
      }
      // Carry over the league the old single-slot dialog remembered.
      const old = localStorage.getItem("pob-redux:trade-league");
      if (old) fallback.league = old;
    } catch {}
    return fallback;
  }

  const saved = load();
  let league = $state(saved.league);
  let realm = $state(saved.realm);
  let status = $state(saved.status);
  let sort = $state<TradeSort>(saved.sort);
  let pages = $state(saved.pages);
  let weights = $state(saved.weights);
  let corrupted = $state(saved.corrupted);
  let mirrored = $state(saved.mirrored);
  let maxPrice = $state(saved.maxPrice);
  let maxPriceType = $state(saved.maxPriceType);
  let maxLevel = $state(saved.maxLevel);
  let sockets = $state(saved.sockets);
  let links = $state(saved.links);
  let jewelType = $state(saved.jewelType);
  let runeBehaviour = $state(saved.runeBehaviour);
  let anointBehaviour = $state(saved.anointBehaviour);
  let eldritch = $state(saved.eldritch);
  let enchantMode = $state(saved.enchantMode);
  let scourge = $state(saved.scourge);
  let talisman = $state(saved.talisman);

  let options = $state<TradeOptions | null>(null);
  let leagues = $state<{ id: string; text: string }[] | null>(null);
  let statuses = $state<{ id: string; label: string }[]>([]);
  let powerStats = $state<PowerStat[]>([]);
  let itemSets = $state<ItemSetInfo[]>([]);
  let socketList = $state<{ nodeId: number; label: string }[]>([]);
  let ringList = $state<{ slot: string; label: string }[]>([]);
  let modLists = $state<Record<string, { value: string; label: string }[]>>({});
  let rates = $state<Record<string, number> | null>(null);
  let ratesFor = "";
  let rows = $state<Row[]>([]);
  let busyKey = $state<string | null>(null);
  let runAll = $state(false);
  let done = $state(0);
  let note = $state("");
  let cancel = false;
  let waitLeft = $state(0);
  let tip = $state<{ tt: Tooltip; x: number; y: number; width: number } | null>(null);
  let tipFor = "";

  const live = $derived(ggg.account?.trade === true);
  const realmId = $derived(game.isPoe2 ? "poe2" : realm);
  const activeWeights = $derived(weights.filter((w) => w.weightMult > 0));
  const slotRows = $derived(rows.filter((r) => !r.special));
  // PoB offers scourge mods only where they still exist.
  const scourgeLeague = $derived(league === "Standard" || league === "Hardcore");

  function remember() {
    const state: Saved = {
      league, realm, status, sort, pages, weights, corrupted, mirrored, maxPrice, maxPriceType, maxLevel, sockets, links,
      jewelType, runeBehaviour, anointBehaviour, eldritch, enchantMode, scourge, talisman,
    };
    try {
      localStorage.setItem(KEY, JSON.stringify(state));
      localStorage.setItem("pob-redux:trade-league", league);
    } catch {}
  }

  function patch(key: string, change: Partial<Row>) {
    rows = rows.map((r) => (r.key === key ? { ...r, ...change } : r));
  }

  async function loadRows(keep: boolean) {
    try {
      const r = await engine.tradeRows();
      socketList = r.sockets;
      ringList = r.rings;
      const old = new Map(keep ? rows.map((row) => [row.key, row]) : []);
      rows = r.rows.map((row) => {
        const before = old.get(row.key);
        const nodeId = before?.nodeId != null && r.sockets.some((s) => s.nodeId === before.nodeId) ? before.nodeId : (r.sockets[0]?.nodeId ?? null);
        return {
          url: "",
          error: null,
          hint: null,
          results: null,
          picked: 0,
          equipSlot: null,
          allMods: false,
          filtersOpen: false,
          required: [],
          blocked: [],
          ...before,
          ...row,
          nodeId: row.special ? nodeId : null,
          ringSlot: row.needs === "ring" ? (before?.ringSlot ?? r.rings[0]?.slot ?? null) : null,
        };
      });
    } catch (e) {
      build.error = String(e);
    }
  }

  async function loadLeagues() {
    try {
      const r = await engine.tradeLeagues(realmId);
      leagues = r.leagues;
      if (!r.leagues.some((l) => l.id === league) && r.leagues.length) league = r.leagues[0].id;
    } catch {
      leagues = null;
    }
    if (live) await privateLeagues();
  }

  /** Signed in, the trade site also lists the private leagues the account is in. */
  async function privateLeagues() {
    try {
      const reply = await gggTradeRequest(`https://www.pathofexile.com/api/${game.isPoe2 ? "trade2" : "trade"}/data/leagues`, null);
      if (reply.kind !== "reply" || reply.status !== 200) return;
      const all = (JSON.parse(reply.body).result ?? []) as { id: string; text?: string; realm?: string }[];
      const mine = all.filter((l) => l.realm === realmId).map((l) => ({ id: l.id, text: l.text ?? l.id }));
      if (mine.length) leagues = mine;
    } catch {}
  }

  /** PoB's currency rates. The price sorts and the order of the total need them. */
  async function loadRates() {
    const key = `${realmId}/${league}`;
    if (!live || ratesFor === key) return;
    ratesFor = key;
    try {
      rates = (await engine.tradeRates({ league, realm: realmId })).rates;
    } catch {
      rates = null;
    }
  }

  onMount(() => {
    void loadRows(false);
    engine.powerStats().then((r) => (powerStats = r.stats)).catch(() => {});
    engine.tradeStatusOptions().then((r) => (statuses = r.options)).catch(() => {});
    engine.listItemSets().then((r) => (itemSets = r.itemSets)).catch(() => {});
    engine
      .tradeOptions()
      .then((o) => {
        options = o;
        if (!o.jewelTypes.includes(jewelType)) jewelType = o.jewelTypes[0];
        if (!o.realms.includes(realm)) realm = o.realms[0];
        if (!o.currencies.some((c) => c.id === maxPriceType)) maxPriceType = "";
      })
      .catch((e) => (build.error = String(e)));
    // PoB keeps the weights in the build; a build without any uses the ones chosen last.
    engine.tradeWeightsGet().then((r) => r.weights.length && (weights = r.weights)).catch(() => {});
    void ggg.sync().then(async () => {
      await loadLeagues();
      await loadRates();
    });
  });

  /** Engine errors arrive with a Lua traceback; only the first line says anything. */
  function friendly(e: unknown) {
    const first = String(e).split("\n")[0].replace(/^EngineError: [a-z_]+: /, "").replace(/^runtime error: /, "").replace(/^Error: /, "");
    if (first.includes("found no mods to search for")) return m.trader_nothing_to_weigh();
    if (first.includes("not supported")) return m.trader_slot_unsupported();
    if (first.includes("No Matching Results")) return m.trader_no_listings();
    return first;
  }

  const searchBase = $derived(
    game.isPoe2 ? "https://www.pathofexile.com/trade2/search/poe2" : `https://www.pathofexile.com/trade/search${realm === "pc" ? "" : `/${realm}`}`,
  );

  function urlFor(query: string) {
    return `${searchBase}/${encodeURIComponent(league)}?q=${encodeURIComponent(query)}`;
  }

  /** A trade site search link PoB can price: league and search id, with or without a realm. */
  function priceable(url: string) {
    return /^https:\/\/www\.pathofexile\.com\/trade2?\/search\/[^/?]+\/[^/?]+(\/[^/?]+)?$/.test(url.trim());
  }

  function target(r: Row): TradeTarget {
    if (!r.special) return { slotName: r.slot ?? "" };
    return { special: r.special, nodeId: r.nodeId ?? undefined, ringSlot: r.ringSlot ?? undefined };
  }

  function modFilters(list: Filter[]) {
    return list.filter((f) => f.tradeId).map((f) => ({ tradeId: f.tradeId, value: f.value.trim() === "" ? undefined : Number(f.value) }));
  }

  /** Every option goes along; the engine applies each one to the slots PoB offers it for. */
  function queryOptions(r: Row): TradeQueryOptions {
    return {
      ...target(r),
      statWeights: activeWeights,
      status,
      includeCorrupted: corrupted,
      includeMirrored: mirrored,
      maxLevel: maxLevel > 0 ? maxLevel : undefined,
      maxPrice: maxPrice > 0 ? maxPrice : undefined,
      maxPriceType: maxPriceType || undefined,
      sockets: sockets > 0 ? sockets : undefined,
      links: links > 0 ? links : undefined,
      jewelType,
      runeBehaviour: options?.runeBehaviours ? runeBehaviour : undefined,
      anointBehaviour: options?.anointBehaviours ? anointBehaviour : undefined,
      includeEldritch: options?.eldritch ? eldritch : undefined,
      copyEnchantMode: options?.enchantModes ? enchantMode : undefined,
      includeScourge: options?.scourge ? scourge && scourgeLeague : undefined,
      includeTalisman: options?.talisman ? talisman : undefined,
      includeAllWEMods: r.allMods || undefined,
      requiredMods: modFilters(r.required),
      blockedMods: modFilters(r.blocked),
    };
  }

  const STOPPED = "stopped";
  const RESULT_INDENT = 148;

  async function pause(seconds: number) {
    for (let left = seconds; left > 0; left--) {
      if (cancel) throw new Error(STOPPED);
      waitLeft = left;
      await new Promise((r) => setTimeout(r, 1000));
    }
    waitLeft = 0;
  }

  /** PoB decides which searches and listings to ask for; each one is sent with the user's sign-in. */
  async function listings(r: Row, source: { query: string } | { url: string }) {
    if (sort === "Price" || sort === "StatValuePrice") await loadRates();
    let state = await engine.tradeResultsStart({ ...target(r), ...source, league, realm: realmId, pages, sort, statWeights: activeWeights });
    const queue = [...state.requests];
    while (!state.done) {
      if (cancel) throw new Error(STOPPED);
      const request = queue.shift();
      if (!request) throw new Error(m.trader_stalled());
      let reply = await gggTradeRequest(request.url, request.body);
      while (reply.kind === "wait") {
        await pause(reply.seconds);
        reply = await gggTradeRequest(request.url, request.body);
      }
      state = await engine.tradeResultsReply({ id: request.id, status: reply.status, body: reply.body });
      queue.push(...state.requests);
    }
    if (state.error) throw new Error(state.error);
    const results = state.results ?? [];
    if (!results.length) throw new Error(m.trader_no_listings());
    const change: Partial<Row> = { results, picked: 0, equipSlot: state.equipSlot, hint: state.unpriced ? m.trader_unpriced() : null };
    if ("query" in source && state.queryId) change.url = `${searchBase}/${encodeURIComponent(league)}/${state.queryId}`;
    patch(r.key, change);
  }

  /** Runs one row with the listings step guarded, so a failed lookup still leaves the search link. */
  async function withListings(r: Row, source: { query: string } | { url: string }): Promise<boolean> {
    try {
      await listings(r, source);
    } catch (e) {
      if (String(e).includes(STOPPED)) return false;
      patch(r.key, { hint: friendly(e) });
      await ggg.sync();
    }
    return true;
  }

  async function generate(key: string): Promise<boolean> {
    const r = rows.find((row) => row.key === key);
    if (!r) return false;
    busyKey = key;
    patch(key, { error: null, hint: null, results: null, picked: 0 });
    try {
      await engine.tradeSearchStart(queryOptions(r));
      for (let i = 0; i < 600; i++) {
        const step = await engine.tradeSearchStep(150);
        if (step.done) break;
      }
      const { query } = await engine.tradeSearchResult();
      patch(key, { url: urlFor(query) });
      return live ? await withListings(r, { query }) : true;
    } catch (e) {
      patch(key, { url: "", error: friendly(e) });
      return false;
    } finally {
      busyKey = null;
      waitLeft = 0;
    }
  }

  async function one(key: string) {
    remember();
    note = "";
    cancel = false;
    await generate(key);
  }

  /** Prices the link in the row as it is, the way PoB's Price Item button does. */
  async function price(key: string) {
    const r = rows.find((row) => row.key === key);
    if (!r || !priceable(r.url)) return;
    remember();
    note = "";
    cancel = false;
    busyKey = key;
    patch(key, { error: null, hint: null, results: null, picked: 0 });
    try {
      await withListings(r, { url: r.url.trim() });
    } finally {
      busyKey = null;
      waitLeft = 0;
    }
  }

  async function all() {
    remember();
    runAll = true;
    cancel = false;
    done = 0;
    note = "";
    let ok = 0;
    const todo = slotRows.map((r) => r.key);
    for (const key of todo) {
      if (cancel) break;
      if (await generate(key)) ok++;
      done++;
    }
    runAll = false;
    note = cancel ? m.trader_stopped({ done, total: todo.length }) : m.trader_summary({ ok, total: todo.length });
  }

  /** The listings each row already has, weighed again: after a sort, weight or build change. */
  async function relist() {
    for (const r of rows) {
      if (!r.results) continue;
      try {
        const again = await engine.tradeResults({ key: r.key, sort, statWeights: activeWeights });
        patch(r.key, { results: again.results, picked: 0, hint: again.unpriced ? m.trader_unpriced() : null });
      } catch (e) {
        patch(r.key, { hint: friendly(e) });
      }
    }
  }

  async function resort() {
    remember();
    if (sort === "Price" || sort === "StatValuePrice") await loadRates();
    await relist();
  }

  async function weightsChanged() {
    remember();
    if (!activeWeights.length) return;
    await build.run(() => engine.tradeWeightsSet(activeWeights));
    await relist();
  }

  async function leagueChanged() {
    remember();
    await loadRates();
    if (sort === "Price" || sort === "StatValuePrice") await relist();
  }

  async function realmChanged() {
    remember();
    await loadLeagues();
    await loadRates();
  }

  async function selectSet(id: number) {
    const r = await build.run(() => engine.selectItemSet(id));
    if (!r) return;
    itemSets = r.itemSets;
    await loadRows(false);
  }

  async function signIn() {
    try {
      if (await ggg.signIn()) {
        await privateLeagues();
        await loadRates();
      }
    } catch (e) {
      build.error = String(e);
    }
  }

  async function importResult(r: Row) {
    const result = r.results?.[r.picked];
    if (!result) return;
    const slot = r.equipSlot ?? r.slot ?? undefined;
    const equipped = await build.run(() => engine.equipItemRaw(result.raw, slot));
    if (!equipped) return;
    note = m.trader_imported({ name: equipped.itemName });
    await loadRows(true);
    await relist();
  }

  const whispers = (result: TradeResult | undefined) => !!result?.whisper && result.priceType !== "~b/o";

  /** A listing with no whisper is bought on the trade site, so that opens a search for the one listing. */
  async function contact(r: Row) {
    const result = r.results?.[r.picked];
    if (!result) return;
    try {
      if (whispers(result)) {
        await writeText(result.whisper ?? "");
        note = m.trader_whisper_copied();
      } else {
        const { query } = await engine.tradeResultQuery({ key: r.key, index: result.index });
        await tradeWindow.open(urlFor(query));
      }
    } catch (e) {
      build.error = String(e);
    }
  }

  async function toggleFilters(r: Row) {
    patch(r.key, { filtersOpen: !r.filtersOpen });
    if (r.filtersOpen || !r.slot) return;
    try {
      const { mods } = await engine.tradeMods({ slotName: r.slot, jewelType });
      modLists = { ...modLists, [r.key]: mods.map((mod) => ({ value: mod.tradeId, label: mod.type === "Explicit" ? mod.text : `${mod.text} (${mod.type})` })) };
    } catch (e) {
      build.error = String(e);
    }
  }

  function setFilters(r: Row, kind: FilterKind, list: Filter[]) {
    patch(r.key, kind === "required" ? { required: list } : { blocked: list });
  }

  function showTip(e: MouseEvent, r: Row) {
    const result = r.results?.[r.picked];
    if (!result) return;
    const key = `${r.key}/${result.index}`;
    tipFor = key;
    const box = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const room = box.left + RESULT_INDENT - 16;
    const width = Math.min(540, Math.max(320, room));
    engine
      .itemTooltip({ raw: result.raw, slotName: r.equipSlot ?? r.slot ?? undefined })
      .then((tt) => {
        if (tipFor === key) tip = { tt, x: Math.max(8, room - width), y: box.top, width };
      })
      .catch(() => {});
  }

  function hideTip() {
    tipFor = "";
    tip = null;
  }

  const signed = (value: number) => `${value >= 0 ? "+" : ""}${value.toFixed(1)}%`;

  function label(result: TradeResult) {
    return `${result.name} · ${result.amount} ${result.currency}${result.score === null ? "" : ` · ${signed(result.score)}`}`;
  }

  const breakdown = (result: TradeResult | undefined) => (result?.stats ?? []).map((s) => `${s.label} ${signed(s.change)}`).join(" · ");

  const sortLabel = $derived<Record<TradeSort, string>>({
    StatValue: m.trader_sort_value(),
    StatValuePrice: m.trader_sort_value_price(),
    Price: m.trader_sort_price(),
    Weight: m.trader_sort_site(),
  });
  const realmLabel = $derived<Record<string, string>>({ pc: m.import_realm_pc(), xbox: m.import_realm_xbox(), sony: m.import_realm_sony() });

  // Dearest currency first when PoB has rates for the league, as its own total orders them.
  const total = $derived.by(() => {
    const sums = new Map<string, number>();
    for (const r of rows) {
      const result = r.results?.[r.picked];
      if (result) sums.set(result.currency, (sums.get(result.currency) ?? 0) + result.amount);
    }
    return [...sums.entries()].sort((a, b) => (rates?.[b[0]] ?? 0) - (rates?.[a[0]] ?? 0) || a[0].localeCompare(b[0]));
  });

  async function copy(url: string) {
    try {
      await writeText(url);
      note = m.trader_link_copied();
    } catch (e) {
      build.error = String(e);
    }
  }

  async function copyAll() {
    const lines = rows.filter((r) => r.url).map((r) => `${r.label}\t${r.url}`);
    if (!lines.length) return;
    try {
      await writeText(lines.join("\n"));
      note = m.trader_links_copied({ count: lines.length });
    } catch (e) {
      build.error = String(e);
    }
  }

  const ready = $derived(rows.filter((r) => r.url).length);
  const busy = $derived(busyKey !== null || runAll);
</script>

<div class="modal">
  <div class="panel dialog trdlg">
    <div class="head">
      <span class="label">{m.trader_title()}</span>
      <span class="dim small">{ggg.account ? m.trader_signed_in({ name: ggg.account.name }) : m.trader_subtitle()}</span>
    </div>

    <div class="opts">
      <div class="orow">
        <span class="olabel">{m.trader_league()}</span>
        {#if leagues}
          <select class="select sm grow" bind:value={league} onchange={leagueChanged}>
            {#each leagues as l (l.id)}
              <option value={l.id}>{l.text}</option>
            {/each}
          </select>
        {:else}
          <input class="input sm grow" bind:value={league} placeholder={m.trader_league_placeholder()} onchange={leagueChanged} />
        {/if}
        {#if options && options.realms.length > 1}
          <select class="select sm" bind:value={realm} onchange={realmChanged} title={m.import_realm()}>
            {#each options.realms as id (id)}
              <option value={id}>{realmLabel[id] ?? id}</option>
            {/each}
          </select>
        {/if}
        {#if itemSets.length > 1}
          <select class="select sm" value={itemSets.find((s) => s.active)?.id} onchange={(e) => selectSet(Number((e.target as HTMLSelectElement).value))} title={m.trader_item_set()} disabled={busy}>
            {#each itemSets as s (s.id)}
              <option value={s.id}>{s.title}</option>
            {/each}
          </select>
        {/if}
        <span class="olabel">{m.trader_listings()}</span>
        <select class="select sm" bind:value={status} title={m.trader_listings_title()}>
          {#each statuses as s (s.id)}
            <option value={s.id}>{s.label}</option>
          {/each}
        </select>
      </div>

      {#if live}
        <div class="orow">
          <span class="olabel">{m.trader_sort()}</span>
          <select class="select sm" bind:value={sort} onchange={resort} title={m.trader_sort_title()}>
            {#each options?.sorts ?? [] as id (id)}
              <option value={id}>{sortLabel[id]}</option>
            {/each}
          </select>
          <span class="olabel wide">{m.trader_pages()}</span>
          <input class="input sm wnum" type="number" min="1" max="10" bind:value={pages} title={m.trader_pages_title()} />
        </div>
      {/if}

      {#each weights as w, i}
        <div class="orow">
          <span class="olabel">{i === 0 ? m.trader_weigh_by() : ""}</span>
          <select
            class="select sm grow"
            value={w.stat}
            onchange={(e) => {
              weights[i] = { ...w, stat: (e.target as HTMLSelectElement).value };
              void weightsChanged();
            }}
          >
            <option value="FullDPS">{m.trader_full_dps()}</option>
            {#each powerStats as s (s.stat)}
              <option value={s.stat}>{s.label}</option>
            {/each}
          </select>
          <input
            class="input sm wnum"
            type="number"
            min="0"
            max="10"
            step="0.1"
            value={w.weightMult}
            title={m.trader_weight_title()}
            onchange={(e) => {
              weights[i] = { ...w, weightMult: Number((e.target as HTMLInputElement).value) };
              void weightsChanged();
            }}
          />
          {#if weights.length > 1}
            <button
              class="x"
              onclick={() => {
                weights = weights.filter((_, j) => j !== i);
                void weightsChanged();
              }}
              title={m.common_remove()}>✕</button
            >
          {:else}
            <span class="x"></span>
          {/if}
        </div>
      {/each}

      <div class="orow wrap">
        <span class="olabel"></span>
        <button
          class="btn sm ghost"
          onclick={() => {
            weights = [...weights, { stat: "TotalEHP", weightMult: 0.5 }];
            void weightsChanged();
          }}>{m.trader_add_stat()}</button
        >
        <label class="chk small"><input type="checkbox" bind:checked={corrupted} /> {m.trader_corrupted()}</label>
        <label class="chk small"><input type="checkbox" bind:checked={mirrored} /> {m.trader_mirrored()}</label>
        <span class="olabel wide">{m.trader_max_price()}</span>
        <input class="input sm wnum" type="number" min="0" bind:value={maxPrice} title={m.trader_max_price_title()} />
        <select class="select sm" bind:value={maxPriceType} title={m.trader_max_price_title()}>
          {#each options?.currencies ?? [] as c (c.id)}
            <option value={c.id}>{c.name}</option>
          {/each}
        </select>
        <span class="olabel wide">{m.trader_max_level()}</span>
        <input class="input sm wnum" type="number" min="0" max="100" bind:value={maxLevel} title={m.trader_max_level_title()} />
      </div>

      <div class="orow wrap">
        <span class="olabel"></span>
        <span class="olabel wide">{m.trader_sockets()}</span>
        <input class="input sm wnum" type="number" min="0" max="6" bind:value={sockets} title={m.trader_sockets_title()} />
        {#if options?.links}
          <span class="olabel">{m.trader_links()}</span>
          <input class="input sm wnum" type="number" min="0" max="6" bind:value={links} title={m.trader_links_title()} />
        {/if}
        <span class="olabel">{m.trader_jewels()}</span>
        <select class="select sm" bind:value={jewelType} title={m.trader_jewels_title()}>
          {#each options?.jewelTypes ?? [] as type (type)}
            <option value={type}>{type}</option>
          {/each}
        </select>
        {#if options?.runeBehaviours}
          <span class="olabel">{m.trader_runes()}</span>
          <select class="select sm" bind:value={runeBehaviour} title={m.trader_runes_title()}>
            {#each options.runeBehaviours as mode (mode)}<option value={mode}>{mode}</option>{/each}
          </select>
        {/if}
        {#if options?.anointBehaviours}
          <span class="olabel">{m.trader_anoint()}</span>
          <select class="select sm" bind:value={anointBehaviour} title={m.trader_anoint_title()}>
            {#each options.anointBehaviours as mode (mode)}<option value={mode}>{mode}</option>{/each}
          </select>
        {/if}
        {#if options?.eldritch}
          <span class="olabel">{m.trader_eldritch()}</span>
          <select class="select sm" bind:value={eldritch} title={m.trader_eldritch_title()}>
            {#each options.eldritch as mode (mode)}<option value={mode}>{mode}</option>{/each}
          </select>
        {/if}
        {#if options?.enchantModes}
          <span class="olabel">{m.trader_enchants()}</span>
          <select class="select sm" bind:value={enchantMode} title={m.trader_enchants_title()}>
            {#each options.enchantModes as mode (mode)}<option value={mode}>{mode}</option>{/each}
          </select>
        {/if}
        {#if options?.scourge && scourgeLeague}
          <label class="chk small"><input type="checkbox" bind:checked={scourge} /> {m.trader_scourge()}</label>
        {/if}
        {#if options?.talisman}
          <label class="chk small" title={m.trader_talisman_title()}><input type="checkbox" bind:checked={talisman} /> {m.trader_talisman()}</label>
        {/if}
      </div>
    </div>

    <div class="side">
      <div class="shead">
        {m.trader_slots()}
        <span class="dim num">{rows.length}</span>
        {#if ready}<span class="dim small">{m.trader_with_search({ count: ready })}</span>{/if}
      </div>
      <div class="scroll">
        {#each rows as r, index (r.key)}
          {#if r.special && !rows[index - 1]?.special}
            <div class="other dim small">{m.trader_other_trades()}</div>
          {/if}
          <div class="trow" class:focus={r.slot !== null && r.slot === focusSlot}>
            <span class="sname" style:color={r.special ? "var(--c-unique)" : undefined}>{r.label}</span>
            {#if r.needs === "ring"}
              <select class="select sm iname" value={r.ringSlot ?? ""} onchange={(e) => patch(r.key, { ringSlot: (e.target as HTMLSelectElement).value })}>
                {#each ringList as ring (ring.slot)}<option value={ring.slot}>{ring.label}</option>{/each}
              </select>
            {:else if r.special}
              <select
                class="select sm iname"
                value={r.nodeId ?? ""}
                title={m.trader_socket_title()}
                onchange={(e) => patch(r.key, { nodeId: Number((e.target as HTMLSelectElement).value) })}
              >
                {#each socketList as socket (socket.nodeId)}<option value={socket.nodeId}>{socket.label}</option>{/each}
              </select>
            {:else}
              <span class="iname" style:color={rarityColor[r.itemRarity ?? ""] ?? "var(--fg-2)"}>{r.itemName ?? m.trader_empty_slot()}</span>
            {/if}
            {#if live || r.url}
              <input
                class="input sm grow mono"
                value={r.url}
                placeholder={live ? m.trader_url_placeholder() : ""}
                oninput={(e) => patch(r.key, { url: (e.target as HTMLInputElement).value })}
                onfocus={(e) => (e.target as HTMLInputElement).select()}
              />
              {#if live}
                <button class="btn sm ghost" onclick={() => price(r.key)} disabled={busy || !priceable(r.url)} title={m.trader_price_title()}>{m.trader_price()}</button>
              {/if}
              <button class="btn sm ghost" onclick={() => copy(r.url)} disabled={!r.url}>{m.common_copy_button()}</button>
              <button class="btn sm" onclick={() => tradeWindow.open(r.url)} disabled={!r.url}>{m.common_open()}</button>
            {:else}
              <span class="grow"></span>
            {/if}
            {#if r.special === "Watcher's Eye"}
              <label class="chk small" title={m.trader_all_mods_title()}>
                <input type="checkbox" checked={r.allMods} onchange={(e) => patch(r.key, { allMods: (e.target as HTMLInputElement).checked })} />
                {m.trader_all_mods()}
              </label>
            {:else if !r.special}
              <button class="btn sm ghost" class:on={r.filtersOpen || r.required.length + r.blocked.length > 0} onclick={() => toggleFilters(r)} title={m.trader_stats_title()}>
                {m.trader_stats()}{#if r.required.length + r.blocked.length}<span class="num">{r.required.length + r.blocked.length}</span>{/if}
              </button>
            {/if}
            <button
              class="btn sm ghost find"
              onclick={() => one(r.key)}
              disabled={busy || (r.needs === "socket" && !socketList.length) || (r.needs === "ring" && !ringList.length)}
              title={r.needs === "socket" && !socketList.length ? m.trader_needs_socket() : undefined}
            >
              {busyKey === r.key ? "…" : r.url ? m.trader_redo() : m.trader_find()}
            </button>
          </div>
          {#if r.filtersOpen}
            <div class="filters" style:padding-left={`${RESULT_INDENT}px`}>
              {#each FILTER_KINDS as kind (kind)}
                {@const list = r[kind]}
                {@const title = kind === "required" ? m.trader_required() : m.trader_blocked()}
                <div class="fcol">
                  <span class="dim small">{title}</span>
                  {#each list as filter, i (i)}
                    <div class="frow">
                      <div class="fsel">
                        <SearchSelect
                          value={filter.tradeId}
                          options={modLists[r.key] ?? []}
                          label={title}
                          onchange={(value) => setFilters(r, kind, list.map((f, j) => (j === i ? { ...f, tradeId: value } : f)))}
                        />
                      </div>
                      <input
                        class="input sm fmin"
                        inputmode="decimal"
                        placeholder={m.trader_min()}
                        value={filter.value}
                        oninput={(e) => setFilters(r, kind, list.map((f, j) => (j === i ? { ...f, value: (e.target as HTMLInputElement).value } : f)))}
                      />
                      <button class="x" onclick={() => setFilters(r, kind, list.filter((_, j) => j !== i))} title={m.common_remove()}>✕</button>
                    </div>
                  {/each}
                  {#if list.length < MAX_FILTERS}
                    <div class="fsel">
                      <SearchSelect
                        value=""
                        options={[{ value: "", label: kind === "required" ? m.trader_add_required() : m.trader_add_blocked() }, ...(modLists[r.key] ?? [])]}
                        label={title}
                        onchange={(value) => value && setFilters(r, kind, [...list, { tradeId: value, value: "" }])}
                      />
                    </div>
                  {/if}
                </div>
              {/each}
            </div>
          {/if}
          {#if r.error}
            <div class="rrow small bad" style:padding-left={`${RESULT_INDENT}px`}>{r.error}</div>
          {:else if r.results?.length}
            {@const pickedResult = r.results[r.picked]}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div class="rrow" style:padding-left={`${RESULT_INDENT}px`} onmouseenter={(e) => showTip(e, r)} onmouseleave={hideTip}>
              <select
                class="select sm grow"
                value={r.picked}
                onchange={(e) => {
                  hideTip();
                  patch(r.key, { picked: Number((e.target as HTMLSelectElement).value) });
                }}
              >
                {#each r.results as result, i (result.index)}
                  <option value={i}>{label(result)}</option>
                {/each}
              </select>
              {#if (pickedResult?.stats.length ?? 0) > 1}
                <span class="dim small parts" title={breakdown(pickedResult)}>{breakdown(pickedResult)}</span>
              {/if}
              <button class="btn sm" onclick={() => importResult(r)} disabled={build.busy > 0} title={m.trader_import_title()}>{m.trader_import()}</button>
              <button class="btn sm ghost contact" onclick={() => contact(r)} title={whispers(pickedResult) ? m.trader_whisper_title() : m.trader_listing_title()}>
                {whispers(pickedResult) ? m.trader_whisper() : m.trader_listing()}
              </button>
            </div>
            {#if r.hint}<div class="rrow dim small" style:padding-left={`${RESULT_INDENT}px`}>{r.hint}</div>{/if}
          {:else if r.hint}
            <div class="rrow dim small" style:padding-left={`${RESULT_INDENT}px`}>{r.hint}</div>
          {/if}
        {:else}
          <div class="dim small pad">{m.trader_no_slots()}</div>
        {/each}
      </div>
    </div>

    {#if !live}
      <p class="note dim small">
        {ggg.account ? m.trader_note_again() : m.trader_note_sign_in()}
        {#if ggg.signingIn}
          <span>{m.import_ggg_waiting()}</span>
          {#if ggg.link}
            <button class="btn sm ghost" title={m.ggg_copy_link_title()} onclick={() => ggg.copyLink().catch((e) => (build.error = String(e)))}>{ggg.linkCopied ? m.common_copied() : m.ggg_copy_link()}</button>
          {/if}
          <button class="btn sm ghost" onclick={() => ggg.cancel()}>{m.common_cancel()}</button>
        {:else}
          <button class="btn sm" onclick={signIn}>{m.import_ggg_sign_in()}</button>
        {/if}
      </p>
    {/if}

    <div class="acts">
      {#if busy}
        <button class="btn ghost sm" onclick={() => (cancel = true)}>{m.common_stop()}</button>
        {#if runAll}
          <div class="prog"><span style:width={`${slotRows.length ? Math.round((done / slotRows.length) * 100) : 0}%`}></span></div>
          <span class="dim num small">{done} / {slotRows.length}</span>
        {:else}
          <span class="sp"></span>
        {/if}
        {#if waitLeft > 0}<span class="small" style:color="var(--warn)">{m.trader_waiting({ seconds: waitLeft })}</span>{/if}
      {:else}
        <button class="btn primary" onclick={all}>{m.trader_find_all()}</button>
        {#if ready}
          <button class="btn sm ghost" onclick={copyAll}>{m.trader_copy_all()}</button>
        {/if}
        {#if note}<span class="dim small">{note}</span>{/if}
        <span class="sp"></span>
        {#if total.length}
          <span class="small">{m.trader_total()} <span class="num">{total.map(([currency, amount]) => `${amount} ${currency}`).join(", ")}</span></span>
        {/if}
      {/if}
      <button class="btn ghost" onclick={onclose} disabled={runAll}>{m.common_close()}</button>
    </div>
  </div>
</div>

{#if tip}
  <PobTooltip lines={tip.tt.lines} header={tip.tt.header} runic={tip.tt.runic} uniqueGem={tip.tt.uniqueGem} itemArt={tip.tt.itemArt} x={tip.x} y={tip.y} width={tip.width} />
{/if}

<style>
  .modal {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: var(--backdrop);
    z-index: 6;
  }
  .trdlg {
    width: 980px;
    max-height: 92vh;
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
  }
  .opts {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .orow {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .orow.wrap {
    flex-wrap: wrap;
  }
  .orow .grow {
    flex: 1;
    min-width: 0;
  }
  .olabel {
    width: 62px;
    flex: none;
    font-size: var(--fs-xs);
    color: var(--fg-2);
  }
  .olabel.wide {
    width: auto;
  }
  .wnum {
    width: 70px;
    text-align: right;
  }
  .orow .x,
  .frow .x {
    width: 18px;
    flex: none;
    background: none;
    border: 0;
    color: var(--fg-3);
    font: inherit;
    cursor: var(--cursor-pointer, pointer);
  }
  .orow button.x:hover,
  .frow button.x:hover {
    color: var(--bad);
  }
  .side {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
  }
  .shead {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 6px 10px;
    font-size: var(--fs-xs);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--fg-2);
    border-bottom: 1px solid var(--line-0);
  }
  .shead .small {
    margin-left: auto;
    text-transform: none;
    letter-spacing: 0;
  }
  .scroll {
    overflow: auto;
    height: 40vh;
  }
  .other {
    padding: 8px 10px 2px;
    border-top: 1px solid var(--line-0);
    margin-top: 4px;
  }
  .trow {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 3px 10px;
  }
  .trow:hover {
    background: var(--bg-hover);
  }
  .trow.focus {
    background: var(--bg-active);
  }
  .sname {
    width: 130px;
    flex: none;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .iname {
    width: 170px;
    flex: none;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-xs);
  }
  .trow .grow {
    flex: 1;
    min-width: 0;
  }
  .trow .num {
    margin-left: 4px;
  }
  .find {
    width: 58px;
    flex: none;
  }
  .rrow {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 10px 5px;
  }
  .rrow .grow {
    flex: 1;
    min-width: 0;
  }
  .rrow.bad {
    color: var(--bad);
  }
  .parts {
    max-width: 230px;
    flex: none;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .contact {
    width: 96px;
    flex: none;
    justify-content: center;
  }
  .filters {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
    padding: 2px 10px 8px;
  }
  .fcol {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .frow {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .fsel {
    flex: 1;
    min-width: 0;
  }
  .fmin {
    width: 56px;
    flex: none;
    text-align: right;
  }
  .pad {
    padding: 8px 10px;
  }
  .note {
    margin: 0;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .acts {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .acts .sp {
    flex: 1;
  }
  .prog {
    flex: 1;
    height: 4px;
    background: var(--bg-2);
    border-radius: 2px;
    overflow: hidden;
  }
  .prog span {
    display: block;
    height: 100%;
    background: var(--accent);
    transition: width 120ms linear;
  }
</style>
