<script lang="ts">
  import { onDestroy, tick, untrack } from "svelte";
  import { readText } from "@tauri-apps/plugin-clipboard-manager";
  import {
    engine,
    type CraftBase,
    type ItemCustomization,
    type ItemCustomizationEdit,
    type ItemDbRow,
    type ItemInfo,
    type ItemSetInfo,
    type SharedItem,
    type SlotsResponse,
    type Tooltip,
    type TooltipLine,
  } from "$lib/engine.svelte";
  import { build } from "$lib/state/build.svelte";
  import { ui } from "$lib/state/ui.svelte";
  import { modKey } from "$lib/keys";
  import Kbd from "$lib/components/Kbd.svelte";
  import { game } from "$lib/state/game.svelte";
  import EquipmentGrid from "$lib/components/EquipmentGrid.svelte";
  import PobText from "$lib/components/PobText.svelte";
  import { stripPobText } from "$lib/pobtext";
  import ItemCustomizationControls from "$lib/components/ItemCustomizationControls.svelte";
  import SearchSelect from "$lib/components/SearchSelect.svelte";
  import ItemFrame from "$lib/components/ItemFrame.svelte";
  import PobTooltip from "$lib/components/PobTooltip.svelte";
  import TraderWindow from "$lib/components/TraderWindow.svelte";
  import BuySimilarDialog from "$lib/components/BuySimilarDialog.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import MoreMenu, { type MenuItem } from "$lib/components/MoreMenu.svelte";
  import { m } from "$lib/paraglide/messages";
  import { confirm } from "$lib/state/confirm.svelte";

  let slotsResp = $state<SlotsResponse | null>(null);
  let items = $state<ItemInfo[]>([]);
  let itemSets = $state<ItemSetInfo[]>([]);
  let selectedItem = $state<number | null>(null);
  let buySimilarFor = $state<number | null>(null);

  let dbTab = $state<"unique" | "rare">("unique");
  let dbQuery = $state("");
  let dbType = $state("");
  let dbRows = $state<ItemDbRow[]>([]);
  let dbTypes = $state<{ type: string; count: number }[]>([]);
  let dbTotal = $state(0);
  let dbLoading = $state(false);
  let dbLoaded = $state(false);
  let dbBusy = false;
  let dbStamp = 0;

  let editOpen = $state(false);
  let editText = $state("");
  let editItemId = $state<number | null>(null);
  let editingPreview = $state(false);
  let editError = $state<string | null>(null);
  let editBusy = $state(false);

  type Preview = Awaited<ReturnType<typeof engine.itemPreview>> & { text: string; customization: ItemCustomization };
  let preview = $state<Preview | null>(null);
  let previewError = $state<string | null>(null);
  let previewLoading = $state(false);
  let previewCommitting = $state(false);
  let affixPending = $state(false);
  let detailLoading = $state(false);
  let previewSlot = $state("");
  let detailPane = $state<HTMLDivElement | undefined>();
  type ScrollHold = {
    pane: HTMLDivElement;
    content: HTMLDivElement;
    top: number;
  };
  let scrollHold: ScrollHold | null = null;
  let pendingPaste = $state<{ text: string; stamp: number } | null>(null);
  let previewStamp = 0;
  let alive = true;
  const generation = untrack(() => build.info!.generation);
  const previewStale = $derived(preview?.rev !== build.rev);
  const itemTransitionBusy = $derived(affixPending || previewLoading || previewCommitting || detailLoading);
  const itemBusy = $derived(itemTransitionBusy || build.busy > 0);

  onDestroy(() => {
    alive = false;
    previewStamp++;
    clearTimeout(tipTimer);
  });

  function discardPreview() {
    previewStamp++;
    pendingPaste = null;
    preview = null;
    previewError = null;
    previewLoading = false;
  }

  function captureDetailScroll(pane = detailPane) {
    if (!pane) return null;
    if (scrollHold?.pane !== pane) {
      scrollHold = { pane, content: pane.querySelector<HTMLDivElement>(".detailcontent")!, top: 0 };
    }
    scrollHold.top = pane.scrollTop;
    scrollHold.content.style.minHeight = `calc(100% + ${Math.ceil(scrollHold.top)}px)`;
    return scrollHold;
  }

  function setAffixPending(pending: boolean) {
    if (pending) captureDetailScroll();
    affixPending = pending;
  }

  function followDetailScroll(event: Event) {
    const hold = scrollHold;
    if (!hold || hold.pane !== event.currentTarget) return;
    hold.top = hold.pane.scrollTop;
    hold.content.style.minHeight = `calc(100% + ${Math.ceil(hold.top)}px)`;
  }

  function holdDetailScroll(pane: HTMLDivElement | undefined) {
    const hold = captureDetailScroll(pane);
    if (!hold) return () => {};
    return () => {
      void tick().then(() => {
        if (!alive || !pane?.isConnected || pane !== detailPane || scrollHold !== hold) return;
        pane.scrollTop = hold.top;
      });
    };
  }

  function releaseDetailScroll(pane = scrollHold?.pane) {
    const hold = scrollHold;
    if (!hold || hold.pane !== pane) return;
    hold.content.style.minHeight = "";
    scrollHold = null;
  }

  $effect(() => {
    const pane = detailPane;
    return () => releaseDetailScroll(pane);
  });

  // The floor only has to outlast the edit; left in place, it keeps blank space below content that shrank.
  $effect(() => {
    if (itemBusy) return;
    void tick().then(() => {
      if (!itemBusy) releaseDetailScroll();
    });
  });

  async function requestPreview(
    text: string, stamp = ++previewStamp, normalise?: boolean, reportUnrecognized = false, updated?: ItemCustomization,
  ) {
    previewLoading = true;
    previewError = null;
    try {
      if (normalise !== undefined) {
        const prepared = await engine.prepareItemPreview(text, generation, normalise);
        if (!alive || stamp !== previewStamp) return false;
        if (prepared.raw == null) {
          if (reportUnrecognized) previewError = m.items_text_unrecognized();
          return false;
        }
        text = prepared.raw;
      }
      if (!text.trim()) throw new Error(m.items_paste_empty());
      let result: Awaited<ReturnType<typeof engine.itemPreview>>;
      let customization: ItemCustomization;
      for (;;) {
        const showDifferences = statDiff;
        const revision = build.rev;
        [result, customization] = await Promise.all([
          engine.itemPreview(text, generation),
          updated?.raw === text ? updated : engine.itemCustomization({ raw: text, generation }),
        ]);
        if (!alive || stamp !== previewStamp) return false;
        if (revision !== build.rev || showDifferences !== statDiff) continue;
        break;
      }
      const pane = updated && preview ? detailPane : undefined;
      const restoreScroll = holdDetailScroll(pane);
      preview = { ...result, text, customization };
      restoreScroll();
      if (!result.slots.some((s) => s.slot === previewSlot)) previewSlot = result.slots[0]?.slot ?? "";
      hideTip();
      return true;
    } catch (e) {
      if (alive && stamp === previewStamp) previewError = String(e);
      return false;
    } finally {
      if (alive && stamp === previewStamp) previewLoading = false;
    }
  }

  async function customizePreview(edit: ItemCustomizationEdit) {
    if (!preview || previewLoading || previewCommitting) return false;
    captureDetailScroll();
    const stamp = ++previewStamp;
    previewLoading = true;
    previewError = null;
    try {
      const updated = await engine.customizeItem({ raw: preview.text, generation }, edit);
      if (alive && stamp === previewStamp) {
        if (await requestPreview(updated.raw, stamp, undefined, false, updated)) return updated;
      }
      return false;
    } catch (e) {
      if (alive && stamp === previewStamp) previewError = String(e);
      return false;
    } finally {
      if (alive && stamp === previewStamp) previewLoading = false;
    }
  }

  async function pasteItem() {
    if (previewCommitting) return;
    const stamp = ++previewStamp;
    pendingPaste = null;
    previewLoading = true;
    previewError = null;
    try {
      const text = (await readText()) ?? "";
      if (alive && stamp === previewStamp) pendingPaste = { text, stamp };
    } catch (e) {
      if (alive && stamp === previewStamp) {
        previewError = m.items_paste_failed({ error: String(e) });
        previewLoading = false;
      }
    }
  }

  $effect(() => {
    const pending = pendingPaste;
    if (!pending || build.busy > 0) return;
    untrack(() => {
      pendingPaste = null;
      if (alive && pending.stamp === previewStamp) void requestPreview(pending.text, pending.stamp, true);
    });
  });

  function editPreview() {
    if (itemTransitionBusy) return;
    editItemId = null;
    editingPreview = true;
    editText = preview?.text ?? "";
    editError = null;
    editOpen = true;
  }

  async function addPreview(equip: boolean) {
    if (!preview || itemBusy || previewStale) return;
    const candidate = preview;
    const slot = previewSlot;
    if (equip && !candidate.slots.some((s) => s.slot === slot)) return;
    previewCommitting = true;
    previewError = null;
    // Clear the committed draft before sync, which can fail independently.
    const result = await build.run(async () => {
      try {
        return equip
          ? await engine.equipItemRaw(candidate.text, slot, generation)
          : await engine.itemEdit(candidate.text, undefined, generation);
      } catch (e) {
        if (alive) previewError = String(e);
        throw e;
      }
    }, { sync: false });
    if (result && alive) {
      discardPreview();
      selectedItem = result.itemId;
    }
    if (result) await build.run(() => build.sync(), { sync: false, user: false });
    previewCommitting = false;
  }

  $effect(() => {
    build.rev;
    statDiff;
    untrack(() => {
      if (preview && !previewCommitting && !previewLoading) void requestPreview(preview.text);
    });
  });

  let craftOpen = $state(false);
  let craftData = $state<{ types: string[]; bases: Record<string, CraftBase[]> } | null>(null);
  let craftType = $state("");
  let craftBase = $state("");
  let craftRarity = $state("RARE");
  let craftTitle = $state("New Item");
  let craftEquip = $state(true);

  let detail = $state<{ itemId: number; tt: Tooltip; customization: ItemCustomization } | null>(null);
  let pendingDetail: { itemId: number; customization: ItemCustomization } | null = null;
  $effect(() => {
    const id = selectedItem;
    build.rev;
    let active = true;
    untrack(() => {
      if (id == null) {
        detail = null;
        detailLoading = false;
        pendingDetail = null;
        return;
      }
      detailLoading = true;
      const customization = pendingDetail?.itemId === id ? pendingDetail.customization : null;
      pendingDetail = null;
      Promise.all([engine.itemTooltip({ itemId: id }), customization ?? engine.itemCustomization({ itemId: id, generation })])
        .then(([tt, customization]) => {
          if (active) {
            const pane = detail?.itemId === id ? detailPane : undefined;
            const restoreScroll = holdDetailScroll(pane);
            detail = { itemId: id, tt, customization };
            restoreScroll();
          }
        })
        .catch(() => {
          if (active) {
            detail = null;
            selectedItem = null;
          }
        })
        .finally(() => { if (active) detailLoading = false; });
    });
    return () => { active = false; };
  });

  async function customizeSavedItem(edit: ItemCustomizationEdit) {
    const itemId = selectedItem;
    if (itemId == null) return;
    captureDetailScroll();
    const result = await build.run(async () => {
      const customization = await engine.customizeItem({ itemId, generation }, edit);
      pendingDetail = { itemId, customization };
      return customization;
    });
    if (!result) {
      if (pendingDetail?.itemId === itemId) pendingDetail = null;
    }
    return result;
  }

  // shared items (main.sharedItemList; app-added ones persisted locally)
  const SHARED_KEY = "pob-redux:shared-items";
  let shared = $state<SharedItem[]>([]);
  $effect(() => {
    build.rev;
    engine.getSharedItems().then((r) => (shared = r.items)).catch(() => (shared = []));
  });
  function persistShared(raw: string, add: boolean) {
    try {
      let raws: string[] = JSON.parse(localStorage.getItem(SHARED_KEY) ?? "[]");
      if (add) raws.push(raw);
      else {
        const i = raws.indexOf(raw);
        if (i >= 0) raws.splice(i, 1);
      }
      localStorage.setItem(SHARED_KEY, JSON.stringify(raws));
    } catch {}
  }
  async function addShared(itemId: number) {
    const r = await engine.addSharedItem({ itemId }).catch(() => null);
    if (r) {
      shared = r.items;
      persistShared(r.items[r.items.length - 1].raw, true);
    }
  }
  async function removeShared(it: SharedItem) {
    const r = await engine.removeSharedItem(it.index).catch(() => null);
    if (r) {
      shared = r.items;
      persistShared(it.raw, false);
    }
  }

  // The Trader window: a weighted trade search per slot, opened in the browser.
  let traderOpen = $state(false);
  let traderFocus = $state<string | null>(null);
  function openTrader(slotName: string | null) {
    traderFocus = slotName;
    traderOpen = true;
  }

  async function openCraft() {
    craftOpen = true;
    if (!craftData) {
      craftData = await engine.craftBases().catch(() => null);
      if (craftData) {
        craftType = craftData.types[0] ?? "";
        craftBase = craftData.bases[craftType]?.[0]?.name ?? "";
      }
    }
  }
  async function doCraft() {
    if (itemBusy) return;
    const r = await build.run(() => engine.craftItem({ type: craftType, baseName: craftBase, rarity: craftRarity, title: craftTitle, equip: craftEquip }));
    if (r) {
      craftOpen = false;
      selectedItem = r.itemId;
    }
  }

  function lineStyle(l: TooltipLine): string {
    if (l.size >= 20) return "font-size:13px;font-weight:600";
    if (l.size >= 16) return "font-size:12px";
    return "font-size:11px";
  }

  let tip = $state<{ tt: Tooltip; x: number; y: number } | null>(null);
  let tipTimer = 0;
  const tipCache = new Map<string, Tooltip>();

  const activeSet = $derived(itemSets.find((s) => s.active));
  const setMenu = $derived<MenuItem[]>([
    { label: m.set_new(), onclick: () => build.run(() => engine.createItemSet()) },
    { label: m.set_copy(), onclick: () => build.run(() => engine.copyItemSet()) },
    {
      label: m.set_rename(),
      onclick: () => {
        const t = prompt(m.items_set_name_prompt(), activeSet?.title ?? "");
        if (t && activeSet) build.run(() => engine.renameItemSet(activeSet.id, t));
      },
    },
    { label: m.set_delete(), danger: true, disabled: itemSets.length <= 1, onclick: () => activeSet && build.run(() => engine.deleteItemSet(activeSet.id)) },
  ]);
  const shownSlots = $derived((slotsResp?.slots ?? []).filter((s) => s.shown && !s.inactive));
  const gearSlots = $derived(shownSlots.filter((s) => s.nodeId == null));
  const socketSlots = $derived(shownSlots.filter((s) => s.nodeId != null));
  const selectedEquippedSlot = $derived(selectedItem == null ? null : (items.find((i) => i.id === selectedItem)?.equippedSlot ?? null));

  $effect(() => {
    build.rev;
    tipCache.clear();
    hideTip();
    let live = true;
    Promise.all([engine.listSlots(), engine.getItems(), engine.listItemSets()]).then(([s, i, sets]) => {
      if (!live) return;
      slotsResp = s;
      items = i.items;
      itemSets = sets.itemSets;
    });
    return () => { live = false; };
  });

  $effect(() => {
    const q = dbQuery;
    const t = dbType;
    const tab = dbTab;
    untrack(() => {
      if (dbBusy) return;
      dbBusy = true;
      const stamp = ++dbStamp;
      dbLoading = !dbLoaded;
      engine
        .itemDbList({ db: tab, query: q, type: t || undefined, limit: 80 })
        .then((r) => {
          if (stamp === dbStamp) {
            dbRows = r.items;
            dbTypes = r.types;
            dbTotal = r.total;
            dbLoaded = true;
          }
        })
        .catch(() => {})
        .finally(() => {
          dbBusy = false;
          dbLoading = false;
        });
    });
  });

  function equipSlot(slot: string, e: Event) {
    if (itemBusy) return;
    const id = Number((e.target as HTMLSelectElement).value);
    build.run(() => engine.equipItem(slot, id));
  }

  function setSlotActive(slot: string, active: boolean) {
    if (itemBusy) return;
    build.run(() => engine.setSlotActive(slot, active));
  }

  function selectItem(id: number | null) {
    hideTip();
    selectedItem = id;
  }

  $effect(() => {
    const j = ui.jump;
    if (j?.view !== "items") return;
    untrack(() => {
      ui.jump = null;
      selectItem(j.item);
    });
  });

  // PoB's own Ctrl+D: the "removing this item will give you" lines in item tooltips.
  let statDiff = $state<boolean | null>(null);
  engine.statDifferences().then((r) => (statDiff = r.show)).catch(() => {});
  async function setStatDiff(show: boolean) {
    try {
      statDiff = (await engine.statDifferences(show)).show;
      tipCache.clear();
      tip = null;
    } catch {}
  }
  let dbSearchEl = $state<HTMLInputElement>();
  function onKey(e: KeyboardEvent) {
    if (e.defaultPrevented) return;
    if (modKey(e) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "f" && !editOpen && !craftOpen && !traderOpen && buySimilarFor == null && !document.querySelector('[aria-modal="true"]')) {
      e.preventDefault();
      dbSearchEl?.focus();
      dbSearchEl?.select();
      return;
    }
    const target = e.target;
    if (target instanceof Element && target.closest('input, textarea, select, [contenteditable]:not([contenteditable="false"]), [role="textbox"], [role="dialog"]')) return;
    if (document.querySelector('[aria-modal="true"]')) return;
    if (editOpen || craftOpen || traderOpen || buySimilarFor != null || confirm.current) return;
    if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "v") {
      e.preventDefault();
      if (!e.repeat) void pasteItem();
      return;
    }
    if (e.ctrlKey && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "d" && statDiff !== null) {
      e.preventDefault();
      void setStatDiff(!statDiff);
    }
  }

  // Beside the hovered row rather than under the pointer, so the row's own
  // buttons and the rows below stay visible.
  let tipRequest = 0;
  function showTip(e: MouseEvent | FocusEvent, key: string, fetch: () => Promise<Tooltip>) {
    clearTimeout(tipTimer);
    const request = ++tipRequest;
    tip = null;
    const row = (e.currentTarget as HTMLElement | null)?.getBoundingClientRect();
    let x = Math.min(("clientX" in e ? e.clientX : 8) + 16, window.innerWidth - 560);
    let y = Math.min(("clientY" in e ? e.clientY : 8) + 12, Math.max(window.innerHeight - 520, 40));
    if (row) {
      x = row.right + 8 + 540 <= window.innerWidth ? row.right + 8 : Math.max(8, row.left - 540 - 8);
      y = row.top;
    }
    tipTimer = window.setTimeout(async () => {
      const cached = tipCache.get(key);
      if (cached) {
        tip = { tt: cached, x, y };
        return;
      }
      try {
        const r = await fetch();
        if (request !== tipRequest) return;
        tipCache.set(key, r);
        tip = { tt: r, x, y };
      } catch {
        if (request === tipRequest) tip = null;
      }
    }, 120);
  }
  function hideTip() {
    tipRequest++;
    clearTimeout(tipTimer);
    tip = null;
  }

  async function openEdit(itemId: number | null) {
    if (itemId == null && previewCommitting) return;
    editError = null;
    editItemId = itemId;
    editingPreview = false;
    if (itemId != null) {
      const r = await engine.itemRaw(itemId).catch(() => null);
      editText = r?.raw ?? "";
    } else {
      editText = "";
    }
    editOpen = true;
  }
  async function saveEdit(asNew: boolean) {
    if (editBusy || itemBusy) return;
    editBusy = true;
    editError = null;
    try {
      if (editItemId == null) {
        if (await requestPreview(editText, undefined, !editingPreview, true)) editOpen = false;
        else {
          editError = previewError;
          previewError = null;
        }
        return;
      }
      const saved = await build.run(async () => {
        try {
          return await engine.itemEdit(editText, asNew ? undefined : (editItemId ?? undefined));
        } catch (e) {
          editError = String(e);
          throw e;
        }
      });
      if (saved) editOpen = false;
      else if (!editError) editError = build.error;
    } catch (e) {
      editError = String(e);
    } finally {
      editBusy = false;
    }
  }

  const rarityColor: Record<string, string> = {
    UNIQUE: "var(--c-unique)",
    RARE: "var(--c-rare)",
    MAGIC: "var(--c-magic)",
    NORMAL: "var(--c-normal)",
    RELIC: "var(--c-gem)",
  };
</script>

<svelte:window onkeydown={onKey} />

{#snippet slotRow(s: SlotsResponse["slots"][number])}
  <div
    class="slot"
    class:jewel={s.nodeId != null}
    role="note"
    onmouseenter={(e) => s.itemId !== 0 && showTip(e, `i${s.itemId}`, () => engine.itemTooltip({ itemId: s.itemId }))}
    onmouseleave={hideTip}
  >
    {#if s.active != null}
      <label class="sname act" title={s.itemId === 0 ? undefined : (s.active ? m.items_slot_on : m.items_slot_off)({ slot: s.label ?? s.slot })}>
        <input type="checkbox" checked={s.active} disabled={s.itemId === 0 || itemBusy} onchange={(e) => setSlotActive(s.slot, (e.target as HTMLInputElement).checked)} />
        {s.label ?? s.slot}
      </label>
    {:else}
      <span class="sname">{s.label ?? s.slot}</span>
    {/if}
    <select class="select" class:off={s.active === false && s.itemId !== 0} value={s.itemId} onchange={(e) => equipSlot(s.slot, e)} disabled={itemBusy} style:color={rarityColor[s.itemRarity ?? ""] ?? undefined}>
      <option value={0}>—</option>
      {#each items.filter((it) => it.compatibleSlots.includes(s.slot)) as it}
        <option value={it.id} style:color={rarityColor[it.rarity ?? ""] ?? undefined}>{it.name}</option>
      {/each}
    </select>
    <button
      class="btn sm ghost find"
      disabled={s.itemId === 0}
      onclick={() => (buySimilarFor = s.itemId)}
      title={m.items_buy_similar_title()}
      aria-label={m.items_buy_similar()}
    >
      <Icon name="magnifying-glass" size={14} />
    </button>
  </div>
{/snippet}

<div class="page">
  <div class="toolbar">
    <select class="select setsel" value={activeSet?.id ?? 1} onchange={(e) => build.run(() => engine.selectItemSet(Number((e.target as HTMLSelectElement).value)))} disabled={build.busy > 0} title={m.items_set_title()}>
      {#each itemSets as s}
        <option value={s.id}>{stripPobText(s.title)}</option>
      {/each}
    </select>
    <MoreMenu label={m.set_menu()} items={setMenu} />
    <span class="vr"></span>
    <div class="wset" role="group" aria-label={m.items_weapon_set()} title={m.items_weapon_set()}>
      <button class="btn sm" class:on={!slotsResp?.useSecondWeaponSet} onclick={() => build.run(() => engine.setWeaponSet(1))}><span class="set1">I</span></button>
      <button class="btn sm" class:on={slotsResp?.useSecondWeaponSet} onclick={() => build.run(() => engine.setWeaponSet(2))}><span class="set2">II</span></button>
    </div>
    <span class="vr"></span>
    <button class="btn sm" onclick={openCraft}>{m.items_craft()}</button>
    <button class="btn sm" onclick={() => openEdit(null)} disabled={previewCommitting}>{m.items_new_from_text()}</button>
    <button class="btn sm" onclick={pasteItem} disabled={previewCommitting} title={m.items_paste_hint()}>{m.items_paste()}</button>
    <button class="btn sm ghost" title={m.items_trader_title()} onclick={() => openTrader(null)}>{m.items_trader()}</button>
    {#if statDiff !== null}
      <span class="vr"></span>
      <label class="chk small" title={m.items_stat_diff_title()}>
        <input type="checkbox" checked={statDiff} onchange={(e) => setStatDiff((e.target as HTMLInputElement).checked)} />
        {m.items_stat_diff()}
      </label>
    {/if}
  </div>

  {#if previewError}<div class="err small pad" role="alert">{previewError}</div>{/if}

  <div class="cols">
    <section class="col slots">
      <div class="panel-head"><span class="label">{m.items_equipment()}</span></div>
      <div class="scroll">
        <EquipmentGrid slots={slotsResp?.slots ?? []} {items} game={game.current} groups={build.skills?.socketGroups ?? []} {selectedItem}
          onselect={selectItem}
          onactive={setSlotActive}
          onitemhover={(event, id) => showTip(event, `i${id}`, () => engine.itemTooltip({ itemId: id }))}
          ongemhover={(event, group, gem) => showTip(event, `g${group}:${gem}`, () => engine.gemTooltip(group, gem))}
          onleave={hideTip} />
        {#each gearSlots as s (s.slot)}
          {@render slotRow(s)}
        {/each}
        {#if socketSlots.length}
          <div class="subhead" title={m.items_jewel_sockets_title()}>
            <span>{m.items_jewel_sockets()}</span>
            <span class="dim num">{socketSlots.length}</span>
          </div>
          {#each socketSlots as s (s.slot)}
            {@render slotRow(s)}
          {/each}
        {/if}
      </div>
    </section>

    <section class="col">
      <div class="panel-head"><span class="label">{m.items_in_build()}</span><span class="dim num">{items.length}</span></div>
      <div class="scroll">
        {#each items as it (it.id)}
          <div
            class="item"
            class:sel={selectedItem === it.id}
            role="note"
            onmouseenter={(e) => showTip(e, `i${it.id}`, () => engine.itemTooltip({ itemId: it.id }))}
            onmouseleave={hideTip}
          >
            <button class="iname" style:color={rarityColor[it.rarity ?? ""] ?? "var(--fg-1)"} onclick={() => selectItem(it.id)}>
              {it.name}
            </button>
            <span class="itag dim">{it.equippedSlot ?? ""}</span>
            <span class="iops">
              {#if !it.equippedSlot && it.primarySlot}
                <button class="mini w" title={m.items_equip_in({ slot: it.primarySlot })} onclick={() => it.primarySlot && build.run(() => engine.equipItem(it.primarySlot!, it.id))}>{m.items_equip()}</button>
              {/if}
              <button class="mini w" onclick={() => openEdit(it.id)}>{m.common_edit()}</button>
              <button class="mini x" title={m.items_delete()} onclick={() => build.run(() => engine.deleteItem(it.id))}>✕</button>
            </span>
          </div>
        {/each}
        {#if items.length === 0}
          <div class="dim small pad">{m.items_none()}</div>
        {/if}
      </div>
      <div class="panel-head">
        <span class="label">{m.items_shared()}</span>
        <span class="dim num">{shared.length}</span>
      </div>
      <div class="scroll sharedlist" title={m.items_shared_title()}>
        {#each shared as it (it.index)}
          <div class="item">
            <span class="iname" style:color={rarityColor[it.rarity ?? ""] ?? "var(--fg-1)"}>{it.name}</span>
            <span class="itag dim">{it.baseName ?? ""}</span>
            <span class="iops">
              <button class="mini w" title={m.items_shared_equip_title()} onclick={() => build.run(() => engine.equipSharedItem(it.index))}>{m.items_equip()}</button>
              <button class="mini x" title={m.items_shared_remove_title()} onclick={() => removeShared(it)}>✕</button>
            </span>
          </div>
        {/each}
        {#if shared.length === 0}
          <div class="dim small pad">{m.items_shared_none()}</div>
        {/if}
      </div>
    </section>

    <section class="col db">
      {#if preview}
        <div class="panel-head">
          <span class="label">{m.items_preview_title()}</span>
          <button class="btn sm ghost" onclick={discardPreview} disabled={itemTransitionBusy}>{m.items_preview_discard()}</button>
        </div>
        <div class="scroll detailpane" bind:this={detailPane} onscroll={followDetailScroll}>
          <div class="detailcontent">
            <p class="dim small">{m.items_preview_note()}</p>
            {#if preview.tooltip.header}
              <ItemFrame lines={preview.tooltip.lines} header={preview.tooltip.header} runic={preview.tooltip.runic} uniqueGem={preview.tooltip.uniqueGem} />
            {:else}
              <div class="ttbox plain">
                {#each preview.tooltip.lines as l}
                  {#if l.sep}<div class="tsep"></div>
                  {:else}<div class="tline" class:tcenter={l.center} style={lineStyle(l)}><PobText text={l.text} /></div>{/if}
                {/each}
              </div>
            {/if}
            <div class="modrow">
              <button class="btn sm" onclick={editPreview} disabled={itemTransitionBusy}>{m.items_preview_edit()}</button>
              <button class="btn sm primary" onclick={() => addPreview(false)} disabled={itemBusy || previewStale}>{m.items_preview_add()}</button>
            </div>
            {#if preview.slots.length}
              <div class="modrow">
                <select class="select" bind:value={previewSlot} aria-label={m.items_preview_slot()} disabled={itemTransitionBusy}>
                  {#each preview.slots as s}<option value={s.slot}>{s.label}</option>{/each}
                </select>
                <button class="btn sm" onclick={() => addPreview(true)} disabled={itemBusy || previewStale}>{m.items_preview_equip()}</button>
              </div>
            {:else}
              <p class="dim small">{m.items_preview_no_slot()}</p>
            {/if}
            <ItemCustomizationControls
              data={preview.customization}
              target={{ raw: preview.text, generation }}
              busy={itemBusy}
              sourceSlot={previewSlot || undefined}
              onchange={customizePreview}
              onpendingchange={setAffixPending}
            />
          </div>
        </div>
      {:else if selectedItem != null && detail?.itemId === selectedItem}
        <div class="panel-head">
          <span class="label">{m.items_item()}</span>
          <button class="btn sm ghost" onclick={() => (buySimilarFor = selectedItem)} title={m.items_buy_similar_title()}>{m.items_buy_similar()}</button>
          <button class="btn sm ghost" onclick={() => selectItem(null)} disabled={itemTransitionBusy}>{m.items_back_to_database()}</button>
        </div>
        <div class="scroll detailpane" bind:this={detailPane} onscroll={followDetailScroll}>
          <div class="detailcontent">
            {#if detail.tt.header}
              <div class="ttbox">
                <ItemFrame lines={detail.tt.lines} header={detail.tt.header} runic={detail.tt.runic} uniqueGem={detail.tt.uniqueGem} itemArt={detail.tt.itemArt} />
              </div>
            {:else}
              <div class="ttbox plain">
                {#each detail.tt.lines as l}
                  {#if l.sep}
                    <div class="tsep"></div>
                  {:else}
                    <div class="tline" class:tcenter={l.center} style={lineStyle(l)}><PobText text={l.text} /></div>
                  {/if}
                {/each}
              </div>
            {/if}

            <ItemCustomizationControls
              data={detail.customization}
              target={{ itemId: selectedItem, generation }}
              busy={itemBusy}
              onchange={customizeSavedItem}
              onpendingchange={setAffixPending}
            />

            <div class="craftsec">
              <div class="label">{m.items_modify()}</div>
              <div class="modrow">
                <button class="btn sm ghost" onclick={() => selectedItem != null && addShared(selectedItem)} disabled={itemBusy}>{m.items_add_to_shared()}</button>
                {#if selectedEquippedSlot}
                  <button class="btn sm ghost" title={m.items_find_upgrades_title()} onclick={() => openTrader(selectedEquippedSlot!)}>{m.items_find_upgrades()}</button>
                {/if}
              </div>
            </div>
          </div>
        </div>
      {:else}
      <div class="panel-head">
        <span class="tabs2">
          <button class="t2" class:on={dbTab === "unique"} onclick={() => { dbTab = "unique"; dbType = ""; }}>{m.items_db_uniques()}</button>
          <button class="t2" class:on={dbTab === "rare"} onclick={() => { dbTab = "rare"; dbType = ""; }}>{m.items_db_rares()}</button>
        </span>
        <span class="dim num">{dbTotal}</span>
      </div>
      <div class="dbbar">
        <span class="hinted">
          <input class="input" placeholder={m.common_search()} bind:value={dbQuery} bind:this={dbSearchEl} />
          {#if !dbQuery}<Kbd keys="Mod+F" hint />{/if}
        </span>
        <select class="select typesel" bind:value={dbType}>
          <option value="">{m.common_all_types()}</option>
          {#each dbTypes as t}
            <option value={t.type}>{t.type} ({t.count})</option>
          {/each}
        </select>
      </div>
      <div class="scroll">
        {#if dbLoading}
          <div class="dim small pad">{m.items_db_parsing()}</div>
        {/if}
        {#each dbRows as row (dbTab + row.name)}
          <div
            class="item"
            role="note"
            onmouseenter={(e) => showTip(e, `d${dbTab}:${row.name}:${build.rev}`, () => engine.itemTooltip({ db: dbTab, name: row.name }))}
            onmouseleave={hideTip}
          >
            <span class="iname" style:color={rarityColor[row.rarity ?? ""] ?? "var(--fg-1)"}>{row.name}</span>
            <span class="itag dim">{row.baseName ?? row.type}</span>
            <span class="iops">
              <button class="mini w" title={m.items_db_equip_title()} onclick={() => build.run(() => engine.itemDbEquip(dbTab, row.name))}>{m.items_equip()}</button>
            </span>
          </div>
        {/each}
        {#if !dbLoading && dbRows.length === 0}
          <div class="dim small pad">{m.items_db_none()}</div>
        {/if}
      </div>
      {/if}
    </section>
  </div>

  {#if craftOpen}
    <div class="modal">
      <div class="panel dialog craftdlg">
        <div class="label">{m.items_craft_title()}</div>
        <div class="crow">
          <span class="clabel">{m.items_rarity()}</span>
          <select class="select" bind:value={craftRarity}>
            <option value="NORMAL">{m.items_rarity_normal()}</option>
            <option value="MAGIC">{m.items_rarity_magic()}</option>
            <option value="RARE">{m.items_rarity_rare()}</option>
            <option value="UNIQUE">{m.items_rarity_unique()}</option>
          </select>
        </div>
        {#if craftRarity === "RARE" || craftRarity === "UNIQUE"}
          <div class="crow"><span class="clabel">{m.items_name()}</span><input class="input" bind:value={craftTitle} /></div>
        {/if}
        <div class="crow">
          <span class="clabel">{m.items_type()}</span>
          <SearchSelect
            value={craftType}
            options={(craftData?.types ?? []).map((t) => ({ value: t, label: t }))}
            label={m.items_type()}
            onchange={(t) => {
              craftType = t;
              craftBase = craftData?.bases[t]?.[0]?.name ?? "";
            }}
          />
        </div>
        <div class="crow">
          <span class="clabel">{m.items_base()}</span>
          <SearchSelect
            value={craftBase}
            options={(craftData?.bases[craftType] ?? []).map((b) => ({ value: b.name, label: b.subType ? `${b.name} (${b.subType})` : b.name }))}
            label={m.items_base()}
            onchange={(b) => (craftBase = b)}
          />
        </div>
        <div class="crow">
          <span class="clabel"></span>
          <label class="chk small"><input type="checkbox" bind:checked={craftEquip} /> {m.items_equip_after()}</label>
        </div>
        <div class="actions">
          <button class="btn primary" onclick={doCraft} disabled={!craftBase || itemBusy}>{m.common_create()}</button>
          <button class="btn ghost" onclick={() => (craftOpen = false)}>{m.common_cancel()}</button>
        </div>
      </div>
    </div>
  {/if}

  {#if editOpen}
    <div class="modal">
      <div class="panel dialog">
        <div class="label">{editItemId != null ? m.items_edit_title() : m.items_new_title()}</div>
        <textarea class="textarea" rows="18" bind:value={editText} placeholder={m.items_edit_placeholder()}></textarea>
        {#if editError}<div class="err small">{editError}</div>{/if}
        <div class="actions">
          {#if editItemId != null}
            <button class="btn" onclick={() => saveEdit(true)} disabled={editBusy || itemBusy}>{m.items_save_as_copy()}</button>
          {/if}
          <button class="btn primary" onclick={() => saveEdit(editItemId == null)} disabled={!editText.trim() || editBusy || itemBusy}>
            {editItemId != null ? m.common_save() : m.items_preview_action()}
          </button>
          <button class="btn ghost" onclick={() => (editOpen = false)} disabled={editBusy}>{m.common_cancel()}</button>
        </div>
      </div>
    </div>
  {/if}

  {#if traderOpen}
    <TraderWindow focusSlot={traderFocus} onclose={() => (traderOpen = false)} />
  {/if}

  {#if buySimilarFor != null}
    <BuySimilarDialog target={{ itemId: buySimilarFor }} onclose={() => (buySimilarFor = null)} />
  {/if}

  {#if tip}
    <PobTooltip lines={tip.tt.lines} header={tip.tt.header} runic={tip.tt.runic} uniqueGem={tip.tt.uniqueGem} itemArt={tip.tt.itemArt} x={tip.x} y={tip.y} />
  {/if}
</div>

<style>
  .page {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
    position: relative;
  }
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--line-0);
    background: var(--bg-1);
  }
  .select:disabled {
    opacity: var(--fade-off);
  }
  .setsel {
    flex: none;
    width: 190px;
    height: 24px;
    font-size: var(--fs-xs);
  }
  .vr {
    width: 1px;
    height: 16px;
    background: var(--line-1);
    margin: 0 4px;
  }
  .wset {
    display: inline-flex;
  }
  .wset .set1,
  .wset .set2 {
    font-family: var(--font-mono);
    font-weight: 600;
  }
  .set1 {
    color: var(--bad);
  }
  .set2 {
    color: var(--ok);
  }
  .subhead {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    padding: 10px 12px 4px;
    font-size: var(--fs-xs);
    font-weight: 600;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--fg-3);
  }
  .sharedlist {
    flex: 0 0 auto;
    max-height: 200px;
  }
  .modrow {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .wset .btn {
    border-radius: 0;
  }
  .wset .btn:first-child {
    border-radius: var(--r-1) 0 0 var(--r-1);
  }
  .wset .btn:last-child {
    border-radius: 0 var(--r-1) var(--r-1) 0;
    border-left: 0;
  }
  .cols {
    flex: 1;
    display: grid;
    grid-template-columns: 340px minmax(0, 1fr) minmax(0, 1fr);
    min-height: 0;
  }
  .col {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-right: 1px solid var(--line-0);
  }
  .col:last-child {
    border-right: 0;
  }
  .scroll {
    flex: 1;
    overflow-y: auto;
  }
  .slot {
    display: grid;
    grid-template-columns: 104px 1fr auto;
    align-items: center;
    gap: 8px;
    padding: 3px 10px;
    border-bottom: 1px solid var(--line-0);
  }
  .slot .select {
    height: 24px;
    font-size: var(--fs-xs);
  }
  .find {
    width: 24px;
    height: 24px;
    padding: 0;
    justify-content: center;
    color: var(--fg-2);
  }
  .find:disabled {
    visibility: hidden;
  }
  .slot.jewel .sname {
    color: var(--fg-3);
  }
  .sname {
    font-size: var(--fs-xs);
    color: var(--fg-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sname.act {
    display: flex;
    align-items: center;
    gap: 5px;
  }
  .sname.act input {
    flex-shrink: 0;
    margin: 0;
  }
  .slot .select.off {
    opacity: 0.5;
  }
  .item {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 10px;
    padding: 4px 10px;
    border-bottom: 1px solid var(--line-0);
    font-size: var(--fs-sm);
  }
  .item:hover {
    background: var(--bg-2);
  }
  .item.sel {
    background: var(--bg-2);
    box-shadow: inset 2px 0 0 var(--fg-0);
  }
  .iname {
    appearance: none;
    border: 0;
    background: none;
    padding: 0;
    font-size: var(--fs-sm);
    text-align: left;
    cursor: default;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .itag {
    font-size: var(--fs-2xs);
    white-space: nowrap;
  }
  .iops {
    display: inline-flex;
    gap: 4px;
    visibility: hidden;
  }
  .item:hover .iops {
    visibility: visible;
  }
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
  .tabs2 {
    display: inline-flex;
    gap: 2px;
  }
  .t2 {
    appearance: none;
    border: 0;
    background: none;
    color: var(--fg-2);
    font-size: var(--fs-xs);
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    cursor: pointer;
    padding: 2px 6px;
    border-radius: 3px;
  }
  .t2.on {
    color: var(--fg-0);
    background: var(--bg-active);
  }
  .dbbar {
    display: flex;
    gap: 6px;
    padding: 8px 10px;
    border-bottom: 1px solid var(--line-0);
  }
  .dbbar .hinted {
    position: relative;
    display: flex;
    flex: 1 1 120px;
    min-width: 90px;
  }
  .dbbar .input {
    width: 100%;
  }
  .typesel {
    flex: 0 1 170px;
    min-width: 0;
    font-size: var(--fs-xs);
  }
  .modal {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: var(--backdrop);
    z-index: 5;
  }
  .dialog {
    width: 560px;
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .dialog .textarea {
    width: 100%;
  }
  .actions {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
  }
  .err {
    color: var(--bad);
  }
  .small {
    font-size: var(--fs-xs);
  }
  .pad {
    padding: 10px 12px;
  }
  .detailpane {
    padding: 0;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .detailcontent {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 10px 12px;
    box-sizing: border-box;
    flex-shrink: 0;
  }
  .ttbox.plain {
    padding: 10px 12px;
    border: 1px solid var(--line-0);
    border-radius: var(--r-2);
    background: var(--bg-1);
    line-height: 1.45;
  }
  .tline {
    color: var(--fg-1);
    white-space: pre-wrap;
  }
  .tcenter {
    text-align: center;
  }
  .tsep {
    height: 1px;
    background: var(--line-1);
    margin: 6px 0;
  }
  .craftsec {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .craftdlg {
    width: 460px;
  }
  .crow {
    display: grid;
    grid-template-columns: 64px 1fr;
    align-items: center;
    gap: 10px;
  }
  .clabel {
    font-size: var(--fs-xs);
    color: var(--fg-2);
    text-align: right;
  }
  .chk {
    display: flex;
    align-items: center;
    gap: 5px;
    color: var(--fg-2);
  }
</style>
