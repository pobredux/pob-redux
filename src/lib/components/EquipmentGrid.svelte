<script lang="ts">
  import type { ItemInfo, SlotInfo, SocketGroup } from "$lib/engine.svelte";
  import { socketedGems, equipmentArea, slotArtBase, socketPosition, visibleEquipment } from "$lib/equipment";
  import { m } from "$lib/paraglide/messages";
  import { slotLabel } from "$lib/pob-labels";
  import { socketArtUrls } from "$lib/item-art";
  import ItemIcon from "./ItemIcon.svelte";
  import Icon from "./Icon.svelte";

  let { slots, items, game, groups = [], areas, selectedItem, targetSlot = null, onselect, onempty, onslotmenu, onactive, onitemhover, ongemhover, onleave, legend = true }:
    { slots: SlotInfo[]; items: ItemInfo[]; game: "poe1" | "poe2"; groups?: SocketGroup[]; areas?: Record<string, string>; selectedItem: number | null; targetSlot?: string | null;
      onselect: (id: number) => void; onempty?: (slot: string) => void; onslotmenu?: (event: MouseEvent, slot: string) => void;
      onactive?: (slot: string, active: boolean) => void;
      onitemhover: (event: MouseEvent | FocusEvent, itemId: number) => void;
      ongemhover: (event: MouseEvent | FocusEvent, group: number, gem: number) => void; onleave: () => void;
      legend?: boolean } = $props();
  let keyboardSlot = $state<string | null>(null);
  const visible = $derived(visibleEquipment(slots));
  const areaOf = (slot: SlotInfo) => areas?.[slot.slot] ?? equipmentArea(slot.slot);
  const body = $derived(visible.filter((slot) => areaOf(slot)));
  const isLimb = (slot: SlotInfo) => /^(Arm|Leg) [12]$/.test(slot.slot);
  const limbs = $derived(visible.filter((slot) => isLimb(slot) && byId.has(slot.itemId)));
  const extras = $derived.by(() => {
    const other = visible.filter((slot) => !areaOf(slot) && !isLimb(slot));
    if (game !== "poe2") return other;
    const order = ["Flask 1", "Charm 1", "Charm 2", "Charm 3", "Flask 2"];
    return other.sort((a, b) => {
      const rank = (slot: string) => order.includes(slot) ? order.indexOf(slot) : order.length;
      return rank(a.slot) - rank(b.slot);
    });
  });
  const byId = $derived(new Map(items.map((item) => [item.id, item])));
  let sockets = $state<Record<string, string>>({});
  $effect(() => {
    const current = game;
    let live = true;
    socketArtUrls(current).then((urls) => { if (live) sockets = urls; });
    return () => { live = false; };
  });
  const socketArt = $derived<Record<string, string | undefined>>({ R: sockets.red, G: sockets.green, B: sockets.blue, W: sockets.white, A: sockets.abyss });
  const socketWhite = $derived(sockets.white);
  const socketLink = $derived(sockets.link);
  const socketRune = $derived(sockets.empty);
  const rarity: Record<string, string> = { UNIQUE: "var(--c-unique)", RELIC: "var(--c-gem)", RARE: "var(--c-rare)", MAGIC: "var(--c-magic)" };
</script>

{#snippet cell(slot: SlotInfo)}
  {@const item = byId.get(slot.itemId)}
  {@const sockets = item?.sockets ?? []}
  {@const gems = game === "poe1" ? socketedGems(slot.slot, sockets, groups) : []}
  <div class="cell" class:occupied={!!item} class:off={!!item && slot.active === false} class:selected={!!item && selectedItem === item.id} class:targeted={!item && targetSlot === slot.slot} class:keyboard={keyboardSlot === slot.slot}
    onfocusin={(event) => { if ((event.target as HTMLElement).matches(":focus-visible")) keyboardSlot = slot.slot; }}
    onfocusout={(event) => { if (!event.currentTarget.contains(event.relatedTarget as Node | null)) keyboardSlot = null; }}
    style:grid-area={areaOf(slot)} style:--rarity={rarity[item?.rarity ?? ""] ?? "var(--line-1)"}>
    <button class="gear" disabled={!item && !onempty} aria-label={`${slotLabel(slot.label ?? slot.slot)}: ${item?.name ?? m.items_empty_slot()}`}
      title={item || !onempty ? undefined : m.items_slot_add({ slot: slotLabel(slot.label ?? slot.slot) })}
      aria-pressed={item ? selectedItem === item.id : undefined}
      onmouseenter={(event) => item && onitemhover(event, item.id)} onmouseleave={onleave}
      onfocus={(event) => item && onitemhover(event, item.id)} onblur={onleave}
      onclick={() => (item ? onselect(item.id) : onempty?.(slot.slot))}
      oncontextmenu={(event) => {
        if (!onslotmenu) return;
        event.preventDefault();
        onslotmenu(event, slot.slot);
      }}>
      {#if item}
        <ItemIcon item={{ game, name: item.title ?? item.name, baseName: item.baseName, rarity: item.rarity }} fallback={item.name} />
      {:else}
        {@const ghost = slotArtBase(game, slot.slot)}
        {#if ghost}
          <span class="ghost"><ItemIcon item={{ game, name: ghost, baseName: ghost, rarity: null }} fallback={slotLabel(slot.label ?? slot.slot)} /></span>
        {:else}
          <span class="empty">{slotLabel(slot.label ?? slot.slot)}</span>
        {/if}
        {#if onempty}<span class="plus" aria-hidden="true"><Icon name="plus" size={12} /></span>{/if}
      {/if}
    </button>
    {#if item && slot.active != null && onactive}
      {@const tip = (slot.active ? m.items_slot_on : m.items_slot_off)({ slot: slotLabel(slot.label ?? slot.slot) })}
      <button class="act" class:on={slot.active} aria-pressed={slot.active} aria-label={tip} title={tip}
        onclick={() => onactive(slot.slot, !slot.active)}></button>
    {/if}
    {#if item}
      {#if game === "poe1" && sockets.length}
        {@const height = Math.ceil(sockets.length / 2) * 24}
        <div class="sockets socket-overlay" style:--h={height} aria-label={m.items_assigned_gems()}>
          {#each sockets as socket, index}
            {@const point = socketPosition(index, sockets.length)}
            {#if index && socket.group === sockets[index - 1].group}
              {@const previous = socketPosition(index - 1, sockets.length)}
              <span class="link" class:vertical={previous.x === point.x} aria-hidden="true"
                style:left={`${(previous.x + point.x) / 2 / 48 * 100}%`} style:top={`${(previous.y + point.y) / 2 / height * 100}%`}
                style:background-image={socketLink ? `url("${socketLink}")` : undefined}></span>
            {/if}
          {/each}
          {#each sockets as socket, index}
            {@const point = socketPosition(index, sockets.length)}
            {@const art = socketArt[socket.colour] ?? socketWhite}
            <span class="socket" aria-hidden="true" style:left={`${point.x / 48 * 100}%`} style:top={`${point.y / height * 100}%`}
              style:background-image={art ? `url("${art}")` : undefined}></span>
          {/each}
          {#each gems as entry, index}
            {#if entry}
              {@const point = socketPosition(index, sockets.length)}
              {@const name = entry.gem.name ?? entry.gem.nameSpec ?? ""}
              <button class="gem" class:disabled={!entry.enabled} aria-label={name}
                style:left={`${point.x / 48 * 100}%`} style:top={`${point.y / height * 100}%`}
                onmouseenter={(event) => ongemhover(event, entry.groupIndex, entry.gem.index)} onmouseleave={onleave}
                onfocus={(event) => ongemhover(event, entry.groupIndex, entry.gem.index)} onblur={onleave}
                onclick={() => onselect(item.id)}>
                <span class="inset-art"><ItemIcon item={{ game, name, baseName: name, rarity: null }} supportGem={entry.gem.support} fallback="◆" /></span>
              </button>
            {/if}
          {/each}
        </div>
      {/if}
      {#if game === "poe2" && item.runes?.length}
        {@const rows = Math.ceil(item.runes.length / 2)}
        <div class="runes socket-overlay" style:grid-template-rows={`repeat(${rows}, minmax(0, 1fr))`}
          style:height={`min(${rows * 32}px, calc(100% - 6px))`} aria-label={m.items_runes()}>
          {#each item.runes as rune, index}
            {@const point = socketPosition(index, item.runes.length)}
            <span class="rune" style:grid-column={item.runes.length === 1 ? "1 / -1" : `${(point.x + 12) / 24}`}
              style:grid-row={`${(point.y + 12) / 24}`} style:background-image={socketRune ? `url("${socketRune}")` : undefined} title={`${index + 1}: ${rune}`}>
              {#if rune !== "None"}
                <span class="inset-art"><ItemIcon item={{ game, name: rune, baseName: rune, rarity: null }} fallback="◆" /></span>
              {/if}
            </span>
          {/each}
        </div>
      {/if}
    {/if}
  </div>
{/snippet}

<div class="equipment">
  <div class="paperdoll" role="group" aria-label={m.items_equipment()}>
    {#each body as slot (slot.slot)}{@render cell(slot)}{/each}
  </div>
  {#if extras.length}
    <div class="extras">{#each extras as slot (slot.slot)}{@render cell(slot)}{/each}</div>
  {/if}
  {#if limbs.length}
    <div class="extras">{#each limbs as slot (slot.slot)}{@render cell(slot)}{/each}</div>
  {/if}
  {#if legend}<div class="legend"><span>{onslotmenu ? m.items_equipment_hint_add() : m.items_equipment_hint()}</span></div>{/if}
</div>

<style>
  .equipment { padding: 16px 12px 10px; border-bottom: 1px solid var(--line-0); background: radial-gradient(ellipse at 50% 35%, var(--bg-2), transparent 75%); }
  .paperdoll { display: grid; grid-template-columns: repeat(8, minmax(0, 1fr)); grid-template-rows: repeat(6, minmax(0, 1fr)); gap: 5px; aspect-ratio: 8 / 6; max-width: 400px; margin: 0 auto; }
  .cell { position: relative; container-type: size; min-width: 0; min-height: 0; border: 1px solid var(--line-1); border-radius: 3px; background: var(--bg-0); }
  .cell.occupied { border-color: color-mix(in srgb, var(--rarity) 40%, var(--line-0)); background: color-mix(in srgb, var(--rarity) 7%, var(--bg-0)); }
  .cell.occupied:hover, .cell:focus-within, .cell.selected { border-color: var(--rarity); box-shadow: 0 0 0 1px var(--rarity); }
  .gear { display: flex; align-items: center; justify-content: center; width: 100%; height: 100%; padding: 3px; background: none; border: 0; color: var(--fg-2); cursor: var(--cursor-pointer, pointer); }
  .gear:disabled { cursor: var(--cursor-arrow, default); }
  .cell:not(.occupied):has(.gear:enabled):hover { border-color: var(--line-2); background: var(--bg-1); }
  .cell:not(.occupied):hover .gear:enabled .empty { opacity: 0.9; }
  .cell.targeted { border-color: var(--focus); box-shadow: 0 0 0 1px var(--focus); }
  .gear:focus-visible, .gem:focus-visible { outline: 2px solid var(--fg-0); outline-offset: 2px; }
  .empty { font-size: 9px; line-height: 1.2; overflow-wrap: anywhere; opacity: 0.55; }
  .ghost { display: flex; align-items: center; justify-content: center; width: 100%; height: 100%; padding: 6%; pointer-events: none; }
  .ghost :global(img) { opacity: 0.14; filter: grayscale(1); transition: opacity 120ms; }
  .ghost :global(.fallback) { font-size: 9px; opacity: 0.55; }
  .cell:hover .ghost :global(img), .cell.targeted .ghost :global(img) { opacity: 0.3; }
  .plus { position: absolute; left: 50%; top: 50%; transform: translate(-50%, -50%); display: grid; place-items: center; width: 18px; height: 18px; border: 1px solid var(--line-2); border-radius: 4px; background: color-mix(in oklab, var(--bg-1) 85%, transparent); color: var(--fg-2); pointer-events: none; }
  .cell:hover .plus, .cell.targeted .plus { color: var(--fg-0); border-color: var(--fg-2); }
  .cell.off .gear { opacity: 0.4; filter: grayscale(1); }
  .act { position: absolute; bottom: 3px; left: 50%; transform: translateX(-50%); width: 12px; height: 12px; padding: 0; border: 1px solid var(--line-2); border-radius: 2px; background: var(--bg-0); }
  .act.on { border-color: var(--ok); background: var(--ok); box-shadow: inset 0 0 0 2px var(--bg-0); }
  .act:hover { border-color: var(--fg-2); }
  .act.on:hover { border-color: var(--ok); }
  .act:focus-visible { outline: 2px solid var(--fg-0); outline-offset: 2px; }
  .socket-overlay { opacity: 0; visibility: hidden; pointer-events: none; }
  .cell:hover .socket-overlay, .cell.keyboard .socket-overlay { opacity: 1; visibility: visible; }
  .sockets { position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(64px, calc(100% - 6px)); width: min(calc(100cqw - 6px), calc((100cqh - 6px) * 48 / var(--h))); aspect-ratio: 48 / var(--h); filter: drop-shadow(0 1px 2px #000); }
  .socket, .link { position: absolute; transform: translate(-50%, -50%); background-size: contain; background-position: center; background-repeat: no-repeat; }
  .socket { width: 37.5%; aspect-ratio: 1; }
  .link { width: 16.6667%; aspect-ratio: 2; }
  .link.vertical { transform: translate(-50%, -50%) rotate(90deg); }
  .runes { position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); place-items: center; width: min(64px, calc(100% - 6px)); }
  .rune { display: flex; align-items: center; justify-content: center; width: 100%; height: 100%; max-width: 32px; max-height: 32px; min-width: 0; min-height: 0; background-size: contain; background-position: center; background-repeat: no-repeat; }
  .inset-art { display: flex; align-items: center; justify-content: center; width: 60%; height: 60%; }
  .gem .inset-art { width: 120%; height: 120%; flex-shrink: 0; }
  .rune .inset-art { width: 90%; height: 90%; flex-shrink: 0; }
  .gem { position: absolute; transform: translate(-50%, -50%); display: flex; align-items: center; justify-content: center; width: 41.6667%; aspect-ratio: 1; padding: 0; background: transparent; border: 0; border-radius: 50%; cursor: var(--cursor-pointer, pointer); pointer-events: auto; }
  .gem.disabled { opacity: 0.45; }
  .extras { display: flex; justify-content: center; flex-wrap: wrap; gap: 5px; margin: 10px auto 0; max-width: 400px; }
  .extras .cell { width: 38px; height: 62px; }
  .legend { display: flex; align-items: center; flex-wrap: wrap; justify-content: space-between; gap: 6px; margin-top: 12px; font-size: 10px; color: var(--fg-3); }
</style>
