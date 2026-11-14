<script lang="ts" module>
  export interface SwitcherOption {
    key: string;
    label: string;
    /** Image for the option's tile; without one the tile shows `number` or the label's initials. */
    art?: string;
    number?: number;
    checked?: boolean;
    hint?: string;
    hintTitle?: string;
    dim?: boolean;
    title?: string;
    action?: boolean;
    onpick: () => void;
  }

  export interface SwitcherSection {
    key: string;
    title?: string;
    options: SwitcherOption[];
  }
</script>

<script lang="ts">
  import { tick } from "svelte";
  import Icon from "./Icon.svelte";

  let {
    title,
    detail,
    art,
    label,
    sections,
    disabled = false,
  }: { title: string; detail: string; art?: string; label: string; sections: SwitcherSection[]; disabled?: boolean } = $props();

  const initials = (name: string) =>
    name
      .replace(/\s*\(.*\)\s*$/, "")
      .split(/[^\p{L}\p{N}]+/u)
      .filter(Boolean)
      .slice(0, 2)
      .map((w) => w[0])
      .join("")
      .toUpperCase();

  let open = $state(false);
  let trigger = $state<HTMLButtonElement>();
  let menu = $state<HTMLDivElement>();
  let pos = $state({ left: 0, top: 0, maxHeight: 0 });

  async function show() {
    if (!trigger) return;
    const r = trigger.getBoundingClientRect();
    const edge = trigger.closest("aside")?.getBoundingClientRect().right ?? r.right;
    pos = { left: edge + 6, top: r.top, maxHeight: window.innerHeight - r.top - 12 };
    open = true;
    await tick();
    (menu?.querySelector<HTMLElement>('[aria-checked="true"]') ?? menu?.querySelector<HTMLElement>("[role^=menuitem]"))?.focus();
  }

  function hide(refocus = true) {
    open = false;
    if (refocus) trigger?.focus();
  }

  function pick(option: SwitcherOption) {
    hide();
    option.onpick();
  }

  function outside(e: PointerEvent) {
    if (open && !trigger?.contains(e.target as Node) && !menu?.contains(e.target as Node)) hide(false);
  }

  function menuKey(e: KeyboardEvent) {
    const items = [...(menu?.querySelectorAll<HTMLElement>("[role^=menuitem]") ?? [])];
    const i = items.indexOf(document.activeElement as HTMLElement);
    const go = (n: number) => {
      e.preventDefault();
      items[(n + items.length) % items.length]?.focus();
    };
    if (e.key === "ArrowDown") go(i + 1);
    else if (e.key === "ArrowUp") go(i - 1);
    else if (e.key === "Home") go(0);
    else if (e.key === "End") go(items.length - 1);
    else if (e.key === "Escape") {
      e.preventDefault();
      hide();
    } else if (e.key === "Tab") hide(false);
  }
</script>

<svelte:window onpointerdown={outside} onresize={() => open && hide(false)} />

{#snippet tile(src: string | undefined, name: string, size: number, number?: number)}
  <span class="tile" class:numtile={!src && number != null} style:width="{size}px" style:height="{size}px">
    {#if src}<img {src} alt="" />{:else if number != null}{number}{:else}<span class="ini">{initials(name)}</span>{/if}
  </span>
{/snippet}

<button
  class="picker"
  class:open
  bind:this={trigger}
  aria-haspopup="menu"
  aria-expanded={open}
  title={label}
  {disabled}
  onclick={() => (open ? hide() : show())}
  onkeydown={(e) => {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    show();
  }}
>
  {@render tile(art, title, 34)}
  <span class="txt">
    <span class="t1">{title}</span>
    <span class="t2">{detail}</span>
  </span>
  <span class="chev"><Icon name="caret-up-down" size={14} /></span>
</button>

{#if open}
  <div
    class="menu"
    role="menu"
    tabindex="-1"
    aria-label={label}
    bind:this={menu}
    style:left="{pos.left}px"
    style:top="{pos.top}px"
    style:max-height="{pos.maxHeight}px"
    onkeydown={menuKey}
  >
    {#each sections as section, si (section.key)}
      {#if si > 0}<div class="sep" role="separator"></div>{/if}
      {#if section.title}<div class="mh">{section.title}</div>{/if}
      {#each section.options as option (option.key)}
        <button
          class="mi"
          class:off={option.dim}
          class:action={option.action}
          role={option.action ? "menuitem" : "menuitemradio"}
          aria-checked={option.action ? undefined : !!option.checked}
          title={option.title}
          onclick={() => pick(option)}
        >
          {#if option.action}
            <span class="tile plus"><Icon name="plus" size={12} /></span>
          {:else}
            {@render tile(option.art, option.label, 24, option.number)}
          {/if}
          <span class="ml">{option.label}</span>
          {#if option.hint}<span class="mk" title={option.hintTitle}>{option.hint}</span>{/if}
          {#if option.checked}<span class="ck"><Icon name="check" size={12} /></span>{/if}
        </button>
      {/each}
    {/each}
  </div>
{/if}

<style>
  .picker {
    appearance: none;
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 8px 6px 6px;
    border: 0;
    border-radius: var(--r-2);
    background: transparent;
    color: var(--fg-0);
    font: inherit;
    text-align: left;
    cursor: var(--cursor-pointer, pointer);
    transition: background 120ms;
  }
  .picker:hover:not(:disabled),
  .picker.open {
    background: var(--bg-hover);
  }
  .picker:focus-visible {
    outline: 1px solid var(--focus);
    outline-offset: -1px;
  }
  .picker:disabled {
    cursor: var(--cursor-arrow, default);
  }
  .tile {
    flex: none;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    overflow: hidden;
    border: 1px solid var(--line-2);
    border-radius: 6px;
    background: var(--bg-3);
  }
  .tile img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .ini {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.02em;
    color: var(--fg-1);
  }
  .numtile {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--fg-2);
  }
  .plus {
    color: var(--fg-2);
    background: transparent;
  }
  .txt {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .t1,
  .t2 {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .t1 {
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--fg-0);
  }
  .t2 {
    font-size: var(--fs-xs);
    color: var(--fg-2);
  }
  .chev {
    flex: none;
    display: grid;
    color: var(--fg-2);
  }
  .menu {
    position: fixed;
    z-index: 60;
    width: 300px;
    display: flex;
    flex-direction: column;
    padding: 4px;
    overflow-y: auto;
    border: 1px solid var(--line-2);
    border-radius: var(--r-2);
    background: var(--bg-1);
    box-shadow: var(--shadow-tooltip);
    scrollbar-width: thin;
  }
  .menu:focus {
    outline: none;
  }
  .mh {
    padding: 6px 8px 4px;
    font-size: var(--fs-xs);
    color: var(--fg-3);
  }
  .mi {
    appearance: none;
    flex: none;
    display: flex;
    align-items: center;
    gap: 9px;
    min-height: 32px;
    padding: 4px 8px;
    border: 0;
    border-radius: var(--r-1);
    background: none;
    color: var(--fg-1);
    font: inherit;
    font-size: var(--fs-sm);
    text-align: left;
    cursor: var(--cursor-pointer, pointer);
  }
  .mi:hover,
  .mi:focus-visible {
    outline: none;
    background: var(--bg-hover);
    color: var(--fg-0);
  }
  .mi[aria-checked="true"] {
    color: var(--fg-0);
  }
  .mi.off {
    opacity: 0.55;
  }
  .mi.action {
    color: var(--fg-2);
  }
  .ml {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .mk {
    font-size: var(--fs-xs);
    color: var(--fg-3);
  }
  .ck {
    display: grid;
    color: var(--fg-0);
  }
  .sep {
    flex: none;
    height: 1px;
    margin: 4px 2px;
    background: var(--line-1);
  }
</style>
