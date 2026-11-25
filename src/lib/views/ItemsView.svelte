<script lang="ts">
  import { scrollHint } from "$lib/scroll-hint";
  import { onDestroy, onMount, tick, untrack } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { readText } from "@tauri-apps/plugin-clipboard-manager";
  import {
    engine,
    type CraftBase,
    type ItemCustomization,
    type ItemCustomizationEdit,
    type ItemDbName,
    type ItemDbRow,
    type ItemInfo,
    type ItemSetInfo,
    type SharedItem,
    type SlotsResponse,
    type SquireSlot,
    type SquireState,
    type Tooltip,
  } from "$lib/engine.svelte";
  import { build } from "$lib/state/build.svelte";
  import { ui } from "$lib/state/ui.svelte";
  import { goTo } from "$lib/state/tabs.svelte";
  import { itemWindow } from "$lib/state/item-window.svelte";
  import { createScrollHold } from "$lib/scroll-hold";
  import { removeTemplate, saveTemplate, SHARED_ITEMS_CHANGED } from "$lib/shared-items";
  import { modKey, shortcutKey } from "$lib/keys";
  import Kbd from "$lib/components/Kbd.svelte";
  import { game } from "$lib/state/game.svelte";
  import EquipmentGrid from "$lib/components/EquipmentGrid.svelte";
  import ContextMenu from "$lib/components/ContextMenu.svelte";
  import { isWickermanSlot, WICKERMAN_AREAS } from "$lib/equipment";
  import { stripPobText } from "$lib/pobtext";
  import { slotLabel } from "$lib/pob-labels";
  import ItemCustomizationControls from "$lib/components/ItemCustomizationControls.svelte";
  import ItemDetailHead from "$lib/components/ItemDetailHead.svelte";
  import ItemDetailPane from "$lib/components/ItemDetailPane.svelte";
  import ItemTooltipBox from "$lib/components/ItemTooltipBox.svelte";
  import SearchSelect from "$lib/components/SearchSelect.svelte";
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
  let squire = $state<SquireState | null>(null);
  let doll = $state<"character" | "squire" | "wickerman">("character");
  const squireOffered = $derived(game.current === "poe2" && (ui.squirePreview || !!squire?.present));
  const squireOpen = $derived(squireOffered && doll === "squire");
  const squireLabels = $derived<Record<string, string>>({
    "Weapon 1": m.squire_slot_weapon(),
    "Weapon 2": m.squire_slot_offhand(),
    Helmet: m.squire_slot_helmet(),
    "Body Armour": m.squire_slot_body(),
    Gloves: m.squire_slot_gloves(),
    Boots: m.squire_slot_boots(),
  });
  const squireSlots = $derived<SquireSlot[]>((squire?.slots ?? []).map((s) => ({ ...s, label: squireLabels[s.slot] ?? s.slot })));
  const squireHeld = $derived(new Set(squireSlots.filter((s) => s.itemId !== 0).map((s) => s.itemId)));
  let buySimilarFor = $state<number | null>(null);

  let dbTab = $state<ItemDbName>("unique");
  let sharedTab = $state(false);
  let dbSlot = $state<string | null>(null);
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
  let detailTransition = $state(false);
  let previewSlot = $state("");
  let detailPane = $state<HTMLDivElement | undefined>();
  const scrollHold = createScrollHold();
  let pendingPaste = $state<{ text: string; stamp: number } | null>(null);
  let previewStamp = 0;
  let alive = true;
  const generation = untrack(() => build.info!.generation);
  const previewStale = $derived(preview?.rev !== build.rev);
  const itemTransitionBusy = $derived(affixPending || previewLoading || previewCommitting || detailTransition);
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

  function setAffixPending(pending: boolean) {
    if (pending) scrollHold.capture(detailPane);
    affixPending = pending;
  }

  $effect(() => {
    const pane = detailPane;
    return () => scrollHold.release(pane);
  });

  // The floor only has to outlast the edit; left in place, it keeps blank space below content that shrank.
  $effect(() => {
    if (itemBusy) return;
    void tick().then(() => {
      if (!itemBusy) scrollHold.release();
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
      const restoreScroll = scrollHold.keep(pane, () => alive && pane === detailPane);
      preview = { ...result, text, customization };
      restoreScroll();
      if (dbSlot && result.slots.some((s) => s.slot === dbSlot)) previewSlot = dbSlot;
      else if (!result.slots.some((s) => s.slot === previewSlot)) previewSlot = result.slots[0]?.slot ?? "";
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
    scrollHold.capture(detailPane);
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
      selectedItem = null;
    }
    if (result) await build.run(() => build.sync(), { sync: false, user: false });
    if (result) followInWindow(result.itemId);
    previewCommitting = false;
  }

  // Only the pop-out follows: inline, the detail would replace the database list being browsed.
  function followInWindow(itemId: number) {
    if (itemWindow.open) void itemWindow.show(itemId);
  }

  async function previewText(load: () => Promise<string>) {
    if (previewCommitting) return;
    const stamp = ++previewStamp;
    previewLoading = true;
    previewError = null;
    try {
      const raw = await load();
      if (alive && stamp === previewStamp) await requestPreview(raw, stamp, true);
    } catch (e) {
      if (alive && stamp === previewStamp) {
        previewError = String(e);
        previewLoading = false;
      }
    }
  }

  // A double-click, or Enter on the focused name, opens the preview; a single click leaves the list in place.
  function previewOnOpen(e: MouseEvent, load: () => Promise<string>) {
    if (e.detail === 0 || e.detail === 2) void previewText(load);
  }

  async function equipFromDb(name: string) {
    const r = await build.run(() => engine.itemDbEquip(dbShown, name, dbSlot ?? undefined));
    if (r) followInWindow(r.itemId);
  }

  async function equipShared(index: number) {
    const r = await build.run(() => engine.equipSharedItem(index, dbSlot ?? undefined));
    if (r) followInWindow(r.itemId);
  }

  $effect(() => {
    build.rev;
    statDiff;
    untrack(() => {
      if (preview && !previewCommitting && !previewLoading) void requestPreview(preview.text);
    });
  });

  type CraftData = { types: string[]; bases: Record<string, CraftBase[]>; noRare: string[] };
  let craftOpen = $state(false);
  let craftAll: CraftData | null = null;
  let craftData = $state<CraftData | null>(null);
  let craftSlot = $state<string | null>(null);
  let craftType = $state("");
  let craftBase = $state("");
  let craftRarity = $state("RARE");
  let craftTitle = $state("New Item");
  let craftEquip = $state(true);
  const craftNoRare = $derived(!!craftData?.noRare.includes(craftType));
  $effect(() => {
    if (craftNoRare && craftRarity === "RARE") craftRarity = "MAGIC";
  });

  // shared items (main.sharedItemList; app-added ones persisted locally)
  let shared = $state<SharedItem[]>([]);
  let sharedRefresh = $state(0);
  const sharedRows = $derived.by(() => {
    const q = dbQuery.trim().toLowerCase();
    return q ? shared.filter((it) => `${it.name} ${it.baseName ?? ""}`.toLowerCase().includes(q)) : shared;
  });
  $effect(() => {
    build.rev;
    sharedRefresh;
    engine.getSharedItems().then((r) => (shared = r.items)).catch(() => (shared = []));
  });
  onMount(() => {
    const off = listen(SHARED_ITEMS_CHANGED, () => sharedRefresh++);
    return () => void off.then((unlisten) => unlisten());
  });
  async function addShared(itemId: number) {
    const items = await saveTemplate(itemId);
    if (items) {
      shared = items;
      templateSaved = itemId;
    }
  }
  let templateSaved = $state<number | null>(null);
  $effect(() => {
    selectedItem;
    build.rev;
    templateSaved = null;
  });
  async function removeShared(it: SharedItem) {
    const items = await removeTemplate(it);
    if (items) shared = items;
  }

  function popOut() {
    if (selectedItem == null) return;
    const id = selectedItem;
    selectItem(null);
    void itemWindow.show(id);
  }

  $effect(() => {
    const id = itemWindow.docked;
    if (id == null) return;
    untrack(() => {
      itemWindow.docked = null;
      if (preview) discardPreview();
      hideTip();
      selectedItem = id;
    });
  });

  // The Trader window: a weighted trade search per slot, opened in the browser.
  let traderOpen = $state(false);
  let traderFocus = $state<string | null>(null);
  function openTrader(slotName: string | null) {
    traderFocus = slotName;
    traderOpen = true;
  }

  async function openCraft(slot: string | null = null, equip = slot != null || craftEquip) {
    craftSlot = slot;
    craftEquip = equip;
    craftOpen = true;
    const loaded = slot ? await engine.craftBases(slot).catch(() => null) : (craftAll ??= await engine.craftBases().catch(() => null));
    if (craftSlot !== slot) return;
    craftData = loaded;
    if (craftData && !craftData.bases[craftType]?.some((b) => b.name === craftBase)) {
      if (!craftData.types.includes(craftType)) craftType = craftData.types[0] ?? "";
      craftBase = craftData.bases[craftType]?.[0]?.name ?? "";
    }
  }
  async function doCraft() {
    if (itemBusy) return;
    const slotName = craftEquip && craftSlot ? craftSlot : undefined;
    const r = await build.run(() => engine.craftItem({ type: craftType, baseName: craftBase, rarity: craftRarity, title: craftTitle, equip: craftEquip, slotName }));
    if (r) {
      craftOpen = false;
      selectedItem = r.itemId;
    }
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
  const gearSlots = $derived(shownSlots.filter((s) => s.nodeId == null && !isWickermanSlot(s)));
  const socketSlots = $derived(shownSlots.filter((s) => s.nodeId != null));
  const characterSlots = $derived((slotsResp?.slots ?? []).filter((s) => !isWickermanSlot(s)));
  const wickermanLabels = $derived([m.wickerman_slot_head(), m.wickerman_slot_torso(), m.wickerman_slot_upper_limb(), m.wickerman_slot_lower_limb()]);
  const wickermanSlots = $derived(shownSlots.filter(isWickermanSlot).map((s, i) => ({ ...s, label: wickermanLabels[i] ?? s.label })));
  const wickermanAreas = $derived(Object.fromEntries(wickermanSlots.map((s, i) => [s.slot, WICKERMAN_AREAS[i]])));
  const wickermanOpen = $derived(wickermanSlots.length > 0 && doll === "wickerman");
  const dbShown = $derived<ItemDbName>(dbTab === "viscera" && !wickermanSlots.length ? "unique" : dbTab);
  const wickermanEmpty = $derived(!items.some((it) => wickermanSlots.some((s) => it.compatibleSlots.includes(s.slot))));
  const selectedEquippedSlot = $derived(selectedItem == null ? null : (items.find((i) => i.id === selectedItem)?.equippedSlot ?? null));

  $effect(() => {
    build.rev;
    tipCache.clear();
    hideTip();
    let live = true;
    Promise.all([engine.listSlots(), engine.getItems(), engine.listItemSets(), engine.getSquire().catch(() => null)]).then(([s, i, sets, sq]) => {
      if (!live) return;
      slotsResp = s;
      items = i.items;
      itemSets = sets.itemSets;
      squire = sq;
    });
    return () => { live = false; };
  });

  const dbSlotInfo = $derived(dbSlot ? characterSlots.find((s) => s.slot === dbSlot) : undefined);
  $effect(() => {
    if (dbSlot && (!dbSlotInfo || items.some((it) => it.id === dbSlotInfo.itemId))) dbSlot = null;
  });

  function findForSlot(slot: string, tab?: ItemDbName) {
    hideTip();
    if (preview && !tab) {
      if (preview.slots.some((s) => s.slot === slot)) previewSlot = slot;
      return;
    }
    if (!tab && dbSlot === slot && selectedItem == null) {
      dbSlot = null;
      return;
    }
    if (preview) discardPreview();
    selectedItem = null;
    sharedTab = false;
    if (tab) dbTab = tab;
    dbType = "";
    dbSlot = slot;
  }

  let slotMenu = $state<{ x: number; y: number; slot: string; unique?: number; rare?: number } | null>(null);
  function openSlotMenu(e: MouseEvent, slot: string) {
    hideTip();
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const fromKeyboard = e.clientX === 0 && e.clientY === 0;
    const menu = { x: fromKeyboard ? r.left + 8 : e.clientX, y: fromKeyboard ? r.bottom : e.clientY, slot };
    slotMenu = menu;
    Promise.all([engine.itemDbList({ db: "unique", slot, limit: 0 }), engine.itemDbList({ db: "rare", slot, limit: 0 })])
      .then(([unique, rare]) => {
        if (slotMenu?.slot === slot) slotMenu = { ...slotMenu, unique: unique.total, rare: rare.total };
      })
      .catch(() => {});
  }
  const slotMenuItems = $derived.by<MenuItem[]>(() => {
    const menu = slotMenu;
    if (!menu) return [];
    const none = m.items_slot_none_fit();
    const equipped = characterSlots.find((s) => s.slot === menu.slot && s.itemId !== 0 && items.some((it) => it.id === s.itemId));
    const list: MenuItem[] = [
      { label: m.items_add_unique(), disabled: menu.unique === 0, title: menu.unique === 0 ? none : undefined, onclick: () => findForSlot(menu.slot, "unique") },
      { label: m.items_add_rare(), disabled: menu.rare === 0, title: menu.rare === 0 ? none : undefined, onclick: () => findForSlot(menu.slot, "rare") },
      { label: m.items_craft(), separator: true, onclick: () => void openCraft(menu.slot) },
    ];
    if (equipped) {
      list.push({
        label: m.items_slot_remove(),
        title: m.items_slot_remove_title(),
        danger: true,
        separator: true,
        onclick: () => build.run(() => engine.equipItem(menu.slot, 0)),
      });
    }
    return list;
  });

  let dbPending = false;
  function loadDb() {
    if (dbBusy) {
      dbPending = true;
      return;
    }
    dbBusy = true;
    dbPending = false;
    const stamp = ++dbStamp;
    dbLoading = !dbLoaded;
    engine
      .itemDbList({ db: dbShown, query: dbQuery, type: dbType || undefined, slot: dbSlot ?? undefined, limit: 80 })
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
        if (dbPending) loadDb();
      });
  }
  $effect(() => {
    dbQuery;
    dbType;
    dbShown;
    dbSlot;
    untrack(loadDb);
  });

  function equipSlot(slot: string, id: number) {
    if (itemBusy) return;
    build.run(() => engine.equipItem(slot, id));
  }

  function slotTip(e: MouseEvent | FocusEvent | HTMLElement, slot: string, itemId: number) {
    showTip(e, `s${slot}:${itemId}`, () => engine.itemTooltip({ itemId, slotName: slot }));
  }

  function slotOptions(slot: string) {
    return [
      { value: "0", label: "—" },
      ...items
        .filter((it) => it.compatibleSlots.includes(slot))
        .map((it) => ({ value: String(it.id), label: it.name, color: rarityColor[it.rarity ?? ""] })),
    ];
  }

  async function equipSquire(slot: string, id: number) {
    if (itemBusy) return;
    const next = await build.run(() => engine.setSquireItem(slot, id));
    if (next) squire = next;
  }

  function squireOptions(s: SquireSlot) {
    return [
      { value: "0", label: "—" },
      ...items.filter((it) => s.options.includes(it.id)).map((it) => ({ value: String(it.id), label: it.name, color: rarityColor[it.rarity ?? ""] })),
    ];
  }

  function itemTip(e: MouseEvent | FocusEvent | HTMLElement, id: number) {
    showTip(e, `i${id}`, () => engine.itemTooltip({ itemId: id }));
  }

  function craftTip(anchor: HTMLElement, baseName: string) {
    const craft = { type: craftType, baseName, rarity: craftRarity, title: craftTitle };
    const slotName = craftSlot ?? undefined;
    showTip(anchor, `c${craftType}:${baseName}:${craftRarity}:${craftTitle}:${slotName ?? ""}`, () => engine.itemTooltip({ craft, slotName }));
  }

  function openSquireSkill() {
    if (squire?.group == null) return;
    goTo("skills", { view: "skills", group: squire.group });
  }

  function setSlotActive(slot: string, active: boolean) {
    if (itemBusy) return;
    build.run(() => engine.setSlotActive(slot, active));
  }

  function selectItem(id: number | null) {
    hideTip();
    if (id != null && itemWindow.open) {
      void itemWindow.show(id);
      return;
    }
    selectedItem = id;
  }
  const shownItem = $derived(selectedItem ?? (itemWindow.open ? itemWindow.itemId : null));

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
    if (modKey(e) && !e.shiftKey && !e.altKey && shortcutKey(e).toLowerCase() === "f" && !editOpen && !craftOpen && !traderOpen && buySimilarFor == null && !document.querySelector('[aria-modal="true"]')) {
      e.preventDefault();
      dbSearchEl?.focus();
      dbSearchEl?.select();
      return;
    }
    const target = e.target;
    if (target instanceof Element && target.closest('input, textarea, select, [contenteditable]:not([contenteditable="false"]), [role="textbox"], [role="dialog"]')) return;
    if (document.querySelector('[aria-modal="true"]')) return;
    if (editOpen || craftOpen || traderOpen || buySimilarFor != null || confirm.current) return;
    if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey && shortcutKey(e).toLowerCase() === "v") {
      e.preventDefault();
      if (!e.repeat) void pasteItem();
      return;
    }
    if (e.ctrlKey && !e.shiftKey && !e.altKey && shortcutKey(e).toLowerCase() === "d" && statDiff !== null) {
      e.preventDefault();
      void setStatDiff(!statDiff);
    }
  }

  // Beside the hovered row rather than under the pointer, so the row's own
  // buttons and the rows below stay visible.
  let tipRequest = 0;
  function showTip(e: MouseEvent | FocusEvent | HTMLElement, key: string, fetch: () => Promise<Tooltip>) {
    clearTimeout(tipTimer);
    const request = ++tipRequest;
    tip = null;
    const row = (e instanceof HTMLElement ? e : (e.currentTarget as HTMLElement | null))?.getBoundingClientRect();
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

  type Column = "slots" | "list";
  const COLUMNS_KEY = "pob-redux:items-columns";
  const SLOTS_WIDTH = 380;
  const MIN_SLOTS = 280;
  const MIN_LIST = 220;
  let colsWidth = $state(0);
  let slotsCol = $state<HTMLElement>();
  let listCol = $state<HTMLElement>();
  let slotsWidth = $state(SLOTS_WIDTH);
  let listShare = $state(0.5);
  let resizing = $state<Column | null>(null);
  try {
    const saved = JSON.parse(localStorage.getItem(COLUMNS_KEY) ?? "{}");
    if (Number.isFinite(saved.slots)) slotsWidth = saved.slots;
    if (Number.isFinite(saved.list)) listShare = saved.list;
  } catch {}

  function saveColumns() {
    try {
      localStorage.setItem(COLUMNS_KEY, JSON.stringify({ slots: slotsWidth, list: listShare }));
    } catch {}
  }

  function resizeColumn(column: Column, px: number) {
    if (column === "slots") {
      slotsWidth = Math.round(Math.max(MIN_SLOTS, Math.min(px, colsWidth - 2 * MIN_LIST)));
      return;
    }
    const rest = colsWidth - (slotsCol?.offsetWidth ?? 0);
    if (rest > 2 * MIN_LIST) listShare = Math.max(MIN_LIST, Math.min(px, rest - MIN_LIST)) / rest;
  }

  function dragColumn(e: PointerEvent, column: Column) {
    const grip = e.currentTarget as HTMLElement;
    const start = (column === "slots" ? slotsCol : listCol)?.getBoundingClientRect().left ?? 0;
    grip.setPointerCapture(e.pointerId);
    resizing = column;
    const move = (ev: PointerEvent) => resizeColumn(column, ev.clientX - start);
    const end = () => {
      resizing = null;
      saveColumns();
      grip.removeEventListener("pointermove", move);
      grip.removeEventListener("pointerup", end);
      grip.removeEventListener("pointercancel", end);
    };
    grip.addEventListener("pointermove", move);
    grip.addEventListener("pointerup", end);
    grip.addEventListener("pointercancel", end);
  }

  function nudgeColumn(e: KeyboardEvent, column: Column) {
    const dir = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
    if (!dir) return;
    e.preventDefault();
    const width = (column === "slots" ? slotsCol : listCol)?.offsetWidth ?? 0;
    resizeColumn(column, width + dir * (e.shiftKey ? 40 : 12));
    saveColumns();
  }

  function resetColumn(column: Column) {
    if (column === "slots") slotsWidth = SLOTS_WIDTH;
    else listShare = 0.5;
    saveColumns();
  }

  const columnPercent = $derived({
    slots: colsWidth ? Math.round((Math.min(slotsWidth, colsWidth * 0.6) / colsWidth) * 100) : 0,
    list: Math.round(listShare * 100),
  });

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
    onmouseenter={(e) => s.itemId !== 0 && slotTip(e, s.slot, s.itemId)}
    onmouseleave={hideTip}
  >
    {#if s.active != null}
      <label class="sname act" title={s.itemId === 0 ? undefined : (s.active ? m.items_slot_on : m.items_slot_off)({ slot: slotLabel(s.label ?? s.slot) })}>
        <input type="checkbox" checked={s.active} disabled={s.itemId === 0 || itemBusy} onchange={(e) => setSlotActive(s.slot, (e.target as HTMLInputElement).checked)} />
        {slotLabel(s.label ?? s.slot)}
      </label>
    {:else}
      <span class="sname">{slotLabel(s.label ?? s.slot)}</span>
    {/if}
    <span class="pick" class:off={s.active === false && s.itemId !== 0}>
      <SearchSelect
        value={String(s.itemId)}
        options={slotOptions(s.slot)}
        label={slotLabel(s.label ?? s.slot)}
        disabled={itemBusy}
        onchange={(id) => equipSlot(s.slot, Number(id))}
        onhover={(id, anchor) => (anchor && Number(id) ? slotTip(anchor, s.slot, Number(id)) : hideTip())}
      />
    </span>
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

{#snippet squireRow(s: SquireSlot)}
  <div class="slot" role="note" onmouseenter={(e) => s.itemId !== 0 && itemTip(e, s.itemId)} onmouseleave={hideTip}>
    <span class="sname">{s.label}</span>
    <span class="pick">
      <SearchSelect
        value={String(s.itemId)}
        options={squireOptions(s)}
        label={s.label ?? s.slot}
        disabled={itemBusy}
        onchange={(id) => equipSquire(s.slot, Number(id))}
        onhover={(id, anchor) => (anchor && Number(id) ? itemTip(anchor, Number(id)) : hideTip())}
      />
    </span>
  </div>
{/snippet}

{#snippet grip(column: Column)}
  <div
    class="grip"
    class:dragging={resizing === column}
    role="slider"
    aria-label={column === "slots" ? m.items_resize_equipment() : m.items_resize_lists()}
    aria-valuenow={columnPercent[column]}
    aria-valuemin={0}
    aria-valuemax={100}
    tabindex="0"
    title={m.items_resize_title()}
    onpointerdown={(e) => dragColumn(e, column)}
    onkeydown={(e) => nudgeColumn(e, column)}
    ondblclick={() => resetColumn(column)}
  ></div>
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
    <button class="btn sm" onclick={() => openCraft()}>{m.items_craft()}</button>
    <button class="btn sm" onclick={() => openEdit(null)} disabled={previewCommitting}>{m.items_new_from_text()}</button>
    <button class="btn sm" onclick={pasteItem} disabled={previewCommitting} title={m.items_paste_hint()}>{m.items_paste()}</button>
    <button class="btn sm" title={m.items_trader_title()} onclick={() => openTrader(null)}><Icon name="storefront" size={13} />{m.items_trader()}</button>
    {#if statDiff !== null}
      <span class="vr"></span>
      <label class="chk small" title={m.items_stat_diff_title()}>
        <input type="checkbox" checked={statDiff} onchange={(e) => setStatDiff((e.target as HTMLInputElement).checked)} />
        {m.items_stat_diff()}
      </label>
    {/if}
  </div>

  {#if previewError}<div class="err small pad" role="alert">{previewError}</div>{/if}

  <div
    class="cols"
    class:resizing={resizing != null}
    bind:clientWidth={colsWidth}
    style:grid-template-columns={`min(${slotsWidth}px, 60%) minmax(0, ${listShare}fr) minmax(0, ${1 - listShare}fr)`}
  >
    <section class="col slots" bind:this={slotsCol}>
      {@render grip("slots")}
      <div class="panel-head">
        {#if squireOffered || wickermanSlots.length}
          {@const characterOpen = !squireOpen && !wickermanOpen}
          <span class="tabs2" role="tablist" aria-label={m.items_equipment()}>
            <button class="t2" role="tab" aria-selected={characterOpen} class:on={characterOpen} onclick={() => (doll = "character")}>{m.squire_character()}</button>
            {#if squireOffered}
              <button class="t2" role="tab" aria-selected={squireOpen} class:on={squireOpen} onclick={() => (doll = "squire")}>{m.squire_title()}</button>
            {/if}
            {#if wickermanSlots.length}
              <button class="t2" role="tab" aria-selected={wickermanOpen} class:on={wickermanOpen} onclick={() => (doll = "wickerman")}>{m.wickerman_title()}</button>
            {/if}
          </span>
        {:else}
          <span class="label">{m.items_equipment()}</span>
        {/if}
      </div>
      {#if wickermanOpen}
      <div class="scroll scroll-hint" {@attach scrollHint}>
        <EquipmentGrid slots={wickermanSlots} areas={wickermanAreas} {items} game={game.current} selectedItem={shownItem} legend={false}
          onselect={selectItem}
          onitemhover={(event, id) => itemTip(event, id)}
          ongemhover={() => {}}
          onleave={hideTip} />
        {#each wickermanSlots as s (s.slot)}
          {@render slotRow(s)}
        {/each}
        <p class="sqnote">{m.wickerman_hint()}</p>
        {#if wickermanEmpty}<p class="sqnote">{m.wickerman_no_items()}</p>{/if}
      </div>
      {:else if squireOpen}
      <div class="scroll scroll-hint" {@attach scrollHint}>
        <div class="squirehead">
          <span class="sqname">{m.squire_title()}</span>
          {#if squire?.level != null}<span class="dim num small">{m.squire_level({ level: squire.level })}</span>{/if}
          {#if squire?.grantedBy?.node}<span class="dim small">{m.squire_granted_by({ node: squire.grantedBy.node })}</span>{/if}
          <span class="grow"></span>
          {#if squire?.group != null}
            <button class="btn sm ghost" title={m.squire_open_skill_title()} onclick={openSquireSkill}>{m.squire_open_skill()}</button>
          {/if}
        </div>
        {#if !squire?.present}<p class="sqnote warn">{m.squire_no_skill()}</p>{/if}
        <EquipmentGrid slots={squireSlots} {items} game={game.current} selectedItem={shownItem} legend={false}
          onselect={selectItem}
          onitemhover={(event, id) => itemTip(event, id)}
          ongemhover={() => {}}
          onleave={hideTip} />
        {#each squireSlots as s (s.slot)}
          {@render squireRow(s)}
        {/each}
        <p class="sqnote">{m.squire_equipment_hint()}</p>
        <div class="subhead"><span>{m.squire_stats()}</span></div>
        <p class="sqnote">{m.squire_stats_pending()}</p>
        <p class="sqnote">{m.squire_note()}</p>
      </div>
      {:else}
      <div class="scroll scroll-hint" {@attach scrollHint}>
        <EquipmentGrid slots={characterSlots} {items} game={game.current} groups={build.skills?.socketGroups ?? []} selectedItem={shownItem}
          targetSlot={preview ? previewSlot : selectedItem == null ? dbSlot : null}
          onselect={(id) => selectItem(selectedItem === id ? null : id)}
          onempty={(slot) => findForSlot(slot)}
          onslotmenu={openSlotMenu}
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
      {/if}
    </section>

    <section class="col" bind:this={listCol}>
      {@render grip("list")}
      <div class="panel-head"><span class="label">{m.items_in_build()}</span><span class="dim num">{items.length}</span></div>
      <div class="scroll scroll-hint" {@attach scrollHint}>
        {#each items as it (it.id)}
          <div
            class="item"
            class:sel={shownItem === it.id}
            role="note"
            onmouseenter={(e) => showTip(e, `i${it.id}`, () => engine.itemTooltip({ itemId: it.id }))}
            onmouseleave={hideTip}
          >
            <button class="iname" style:color={rarityColor[it.rarity ?? ""] ?? "var(--fg-1)"} onclick={() => selectItem(it.id)}>
              {it.name}
            </button>
            <span class="itag dim">{it.equippedSlot ?? (squireHeld.has(it.id) ? m.squire_tag() : "")}</span>
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
    </section>

    <section class="col db">
      {#if preview}
        <div class="panel-head">
          <span class="label">{m.items_preview_title()}</span>
          <button class="btn sm ghost" onclick={discardPreview} disabled={itemTransitionBusy}>{m.items_preview_discard()}</button>
        </div>
        <div class="scroll detailpane scroll-hint" {@attach scrollHint} bind:this={detailPane} onscroll={scrollHold.follow}>
          <div class="detailcontent">
            <p class="dim small">{m.items_preview_note()}</p>
            <ItemTooltipBox tt={preview.tooltip} art={false} />
            <div class="modrow">
              <button class="btn sm" onclick={editPreview} disabled={itemTransitionBusy}>{m.items_preview_edit()}</button>
              <button class="btn sm primary" onclick={() => addPreview(false)} disabled={itemBusy || previewStale}>{m.items_preview_add()}</button>
            </div>
            {#if preview.slots.length}
              <div class="modrow">
                <select class="select" bind:value={previewSlot} aria-label={m.items_preview_slot()} disabled={itemTransitionBusy}>
                  {#each preview.slots as s}<option value={s.slot}>{slotLabel(s.label)}</option>{/each}
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
      {:else if selectedItem != null}
        <ItemDetailHead
          saved={templateSaved === selectedItem}
          busy={itemBusy}
          onsave={() => selectedItem != null && addShared(selectedItem)}
          onbuy={() => (buySimilarFor = selectedItem)}
          move={{ icon: "arrow-square-out", title: m.items_pop_out_title(), onclick: popOut }}
          close={{ title: m.items_back_to_database(), disabled: itemTransitionBusy, onclick: () => selectItem(null) }}
        />
        <ItemDetailPane
          itemId={selectedItem}
          {generation}
          rev={build.rev}
          busy={build.busy > 0}
          bind:transitioning={detailTransition}
          onmissing={() => selectItem(null)}
        >
          {#if selectedEquippedSlot}
            <div class="craftsec">
              <div class="label">{m.items_modify()}</div>
              <div class="modrow">
                <button class="btn sm ghost" title={m.items_find_upgrades_title()} onclick={() => openTrader(selectedEquippedSlot!)}>{m.items_find_upgrades()}</button>
              </div>
            </div>
          {/if}
        </ItemDetailPane>
      {:else}
      <div class="panel-head dbhead">
        <span class="tabs2">
          <button class="t2" class:on={!sharedTab && dbShown === "unique"} onclick={() => { dbTab = "unique"; sharedTab = false; dbType = ""; }}>{m.items_db_uniques()}</button>
          <button class="t2" class:on={!sharedTab && dbShown === "rare"} onclick={() => { dbTab = "rare"; sharedTab = false; dbType = ""; }}>{m.items_db_rares()}</button>
          {#if wickermanSlots.length}
            <button class="t2" class:on={!sharedTab && dbShown === "viscera"} onclick={() => { dbTab = "viscera"; sharedTab = false; dbType = ""; }}>{m.items_db_viscera()}</button>
          {/if}
          <button class="t2" class:on={sharedTab} title={m.items_shared_title()} onclick={() => (sharedTab = true)}>{m.items_shared()}</button>
        </span>
        <span class="dim num">{sharedTab ? sharedRows.length : dbTotal}</span>
      </div>
      <div class="dbbar">
        <span class="hinted">
          <input class="input" placeholder={m.common_search()} bind:value={dbQuery} bind:this={dbSearchEl} />
          {#if !dbQuery}<Kbd keys="Mod+F" hint />{/if}
        </span>
        {#if sharedTab}
          <button class="btn sm" onclick={() => openCraft(null, false)}>{m.items_craft()}</button>
        {:else}
          <select class="select typesel" bind:value={dbType}>
            <option value="">{m.common_all_types()}</option>
            {#each dbTypes as t}
              <option value={t.type}>{t.type} ({t.count})</option>
            {/each}
          </select>
        {/if}
      </div>
      {#if dbSlot}
        <div class="slotbar">
          <span>{m.items_db_for_slot({ slot: slotLabel(dbSlotInfo?.label ?? dbSlot) })}</span>
          <button class="mini x" title={m.items_db_clear_slot()} aria-label={m.items_db_clear_slot()} onclick={() => (dbSlot = null)}>✕</button>
        </div>
      {/if}
      <div class="scroll scroll-hint" {@attach scrollHint}>
        {#if sharedTab}
          {#each sharedRows as it (it.index)}
            <div
              class="item"
              role="note"
              onmouseenter={(e) => showTip(e, `h${it.raw}`, () => engine.itemTooltip({ raw: it.raw }))}
              onmouseleave={hideTip}
            >
              <button class="iname" style:color={rarityColor[it.rarity ?? ""] ?? "var(--fg-1)"} onclick={(e) => previewOnOpen(e, async () => it.raw)}>{it.name}</button>
              <span class="itag dim">{it.baseName ?? ""}</span>
              <span class="iops">
                <button class="mini w" title={m.items_shared_equip_title()} onclick={() => equipShared(it.index)}>{m.items_equip()}</button>
                <button class="mini x" title={m.items_shared_remove_title()} onclick={() => removeShared(it)}>✕</button>
              </span>
            </div>
          {/each}
          {#if sharedRows.length === 0}
            <div class="dim small pad">{shared.length ? m.items_db_none() : m.items_shared_none()}</div>
          {/if}
        {:else}
        {#if dbLoading}
          <div class="dim small pad">{m.items_db_parsing()}</div>
        {/if}
        {#each dbRows as row (dbShown + row.name)}
          <div
            class="item"
            role="note"
            onmouseenter={(e) => showTip(e, `d${dbShown}:${row.name}:${build.rev}`, () => engine.itemTooltip({ db: dbShown, name: row.name }))}
            onmouseleave={hideTip}
          >
            <button class="iname" style:color={rarityColor[row.rarity ?? ""] ?? "var(--fg-1)"} onclick={(e) => previewOnOpen(e, async () => (await engine.itemDbRaw(dbShown, row.name)).raw)}>{row.name}</button>
            <span class="itag dim">{row.baseName ?? row.type}</span>
            <span class="iops">
              <button class="mini w" title={m.items_db_equip_title()} onclick={() => equipFromDb(row.name)}>{m.items_equip()}</button>
            </span>
          </div>
        {/each}
        {#if !dbLoading && dbRows.length === 0}
          <div class="dim small pad">{m.items_db_none()}</div>
        {/if}
        {/if}
      </div>
      {/if}
    </section>
  </div>

  {#if craftOpen}
    <div class="modal">
      <div class="panel dialog craftdlg">
        <div class="label">{m.items_craft_title()}</div>
        {#if craftSlot}
          <div class="dim small">{m.items_db_for_slot({ slot: slotLabel(characterSlots.find((s) => s.slot === craftSlot)?.label ?? craftSlot) })}</div>
        {/if}
        <div class="crow">
          <span class="clabel">{m.items_rarity()}</span>
          <select class="select" bind:value={craftRarity}>
            <option value="NORMAL">{m.items_rarity_normal()}</option>
            <option value="MAGIC">{m.items_rarity_magic()}</option>
            {#if !craftNoRare}<option value="RARE">{m.items_rarity_rare()}</option>{/if}
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
            onhover={(b, anchor) => (anchor && b ? craftTip(anchor, b) : hideTip())}
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

  {#if slotMenu}
    <ContextMenu x={slotMenu.x} y={slotMenu.y} label={m.items_slot_menu()} items={slotMenuItems} onclose={() => (slotMenu = null)} />
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
    min-height: 0;
  }
  .cols.resizing {
    cursor: col-resize;
    user-select: none;
  }
  .col {
    position: relative;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    border-right: 1px solid var(--line-0);
  }
  .grip {
    position: absolute;
    top: 0;
    bottom: 0;
    right: -3px;
    width: 7px;
    z-index: 2;
    cursor: col-resize;
    touch-action: none;
  }
  .grip:hover::after,
  .grip:focus-visible::after,
  .grip.dragging::after {
    content: "";
    position: absolute;
    inset: 0 3px;
    background: var(--focus);
  }
  .grip:focus-visible {
    outline: none;
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
  .pick {
    min-width: 0;
  }
  .pick :global(.trigger) {
    height: 24px;
  }
  .pick :global(.trigger:disabled) {
    opacity: var(--fade-off);
  }
  .pick.off :global(.trigger) {
    opacity: 0.5;
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
    cursor: var(--cursor-arrow, default);
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
    cursor: var(--cursor-pointer, pointer);
    border-radius: 3px;
  }
  .mini:hover {
    color: var(--fg-0);
  }
  .mini.x:hover {
    color: var(--bad);
  }
  .squirehead {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--line-0);
  }
  .sqname {
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--fg-0);
  }
  .grow {
    flex: 1;
  }
  .sqnote {
    margin: 0;
    padding: 6px 12px;
    font-size: var(--fs-xs);
    color: var(--fg-3);
  }
  .sqnote.warn {
    color: var(--warn);
  }
  .tabs2 {
    display: inline-flex;
    flex-wrap: wrap;
    gap: 2px;
    min-width: 0;
  }
  .panel-head.dbhead {
    height: auto;
    min-height: 30px;
    padding-block: 4px;
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
    white-space: nowrap;
    flex-shrink: 0;
    cursor: var(--cursor-pointer, pointer);
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
  .slotbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 6px 4px 12px;
    border-bottom: 1px solid var(--line-0);
    background: color-mix(in oklab, var(--focus) 8%, transparent);
    color: var(--focus);
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
