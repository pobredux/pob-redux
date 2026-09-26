<script lang="ts">
  import { chat, experimentDelta, MAX_WIDTH, MIN_WIDTH, type ToolTurn } from "$lib/state/chat.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import ModelPicker from "$lib/components/ModelPicker.svelte";
  import TraitsPicker from "$lib/components/TraitsPicker.svelte";
  import ChangesPicker from "$lib/components/ChangesPicker.svelte";
  import ContextMeter from "$lib/components/ContextMeter.svelte";
  import WorkingTimer from "$lib/components/WorkingTimer.svelte";
  import Markdown from "$lib/components/Markdown.svelte";
  import { build } from "$lib/state/build.svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { m } from "$lib/paraglide/messages";

  let scroller = $state<HTMLDivElement | undefined>();
  let box = $state<HTMLTextAreaElement | undefined>();
  let slashIndex = $state(0);

  // `/` at the start of an empty-ish line opens the tool menu.
  const slashQuery = $derived(
    chat.input.startsWith("/") ? chat.input.slice(1).trim().toLowerCase() : null,
  );
  const slashHits = $derived(
    slashQuery === null
      ? []
      : chat.toolNames.filter((t) => t.name.includes(slashQuery.replace(/\s+/g, "_"))).slice(0, 8),
  );

  /** Follow new output only while the reader is already at the bottom. */
  let stuck = $state(true);
  $effect(() => {
    void chat.turns;
    void chat.busy;
    if (stuck && scroller) scroller.scrollTop = scroller.scrollHeight;
  });

  function onScroll() {
    if (scroller) stuck = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 40;
  }

  function toEnd() {
    stuck = true;
    scroller?.scrollTo({ top: scroller.scrollHeight, behavior: "smooth" });
  }

  let expanded = $state<Record<string, boolean>>({});
  const pretty = (v: unknown) => {
    const t = typeof v === "string" ? v : JSON.stringify(v, null, 2) ?? "";
    return t.length > 20000 ? `${t.slice(0, 20000)}…` : t;
  };

  $effect(() => {
    void slashHits.length;
    slashIndex = 0;
  });

  const moved = $derived(chat.experiment ? experimentDelta(chat.experiment) : []);
  const signed = (n: number, digits = 0) => `${n > 0 ? "+" : ""}${n.toFixed(digits)}`;

  function pickTool(name: string) {
    chat.input = `Use ${name} and tell me what it returns.`;
    box?.focus();
  }

  function onKeydown(e: KeyboardEvent) {
    if (slashHits.length) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        slashIndex = (slashIndex + 1) % slashHits.length;
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        slashIndex = (slashIndex - 1 + slashHits.length) % slashHits.length;
        return;
      }
      if (e.key === "Tab" || (e.key === "Enter" && !e.shiftKey)) {
        e.preventDefault();
        pickTool(slashHits[slashIndex].name);
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        chat.input = "";
        return;
      }
    }
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      chat.send();
    }
  }

  // Drag the left edge. Pointer capture keeps events coming even when the
  // cursor outruns the grip, which it will at speed.
  let dragging = $state(false);
  function startResize(e: PointerEvent) {
    const grip = e.currentTarget as HTMLElement;
    grip.setPointerCapture(e.pointerId);
    dragging = true;
    const startX = e.clientX;
    const startWidth = chat.width;

    const move = (ev: PointerEvent) => chat.setWidth(startWidth + (startX - ev.clientX));
    const end = () => {
      dragging = false;
      chat.setWidth(chat.width, true);
      grip.releasePointerCapture(e.pointerId);
      grip.removeEventListener("pointermove", move);
      grip.removeEventListener("pointerup", end);
      grip.removeEventListener("pointercancel", end);
    };
    grip.addEventListener("pointermove", move);
    grip.addEventListener("pointerup", end);
    grip.addEventListener("pointercancel", end);
  }

  /** Keyboard resize, so the grip is not mouse-only. */
  function gripKey(e: KeyboardEvent) {
    const step = e.shiftKey ? 40 : 12;
    if (e.key === "ArrowLeft") {
      e.preventDefault();
      chat.setWidth(chat.width + step, true);
    } else if (e.key === "ArrowRight") {
      e.preventDefault();
      chat.setWidth(chat.width - step, true);
    }
  }

  let copied = $state(-1);
  let copyTimer: ReturnType<typeof setTimeout> | undefined;
  async function copyText(index: number, text: string) {
    try {
      await writeText(text);
      copied = index;
      clearTimeout(copyTimer);
      copyTimer = setTimeout(() => (copied = -1), 1400);
    } catch (e) {
      chat.error = `Could not copy: ${e}`;
    }
  }

  let detailsCopied = $state(false);
  async function copyDetails() {
    try {
      await writeText(chat.diagnostics());
      detailsCopied = true;
      setTimeout(() => (detailsCopied = false), 1400);
    } catch (e) {
      chat.error = `Could not copy: ${e}`;
    }
  }

  const summary = (t: ToolTurn) => {
    const a = t.args as Record<string, unknown> | null;
    if (!a) return "";
    return Object.entries(a)
      .slice(0, 3)
      .map(([k, v]) => `${k}=${typeof v === "object" ? JSON.stringify(v) : String(v)}`)
      .join(" ");
  };
</script>

<aside class="chat" style:width="{chat.width}px">
  <div
    class="grip"
    class:dragging
    role="slider"
    aria-orientation="vertical"
    aria-label={m.chat_resize()}
    aria-valuenow={chat.width}
    aria-valuemin={MIN_WIDTH}
    aria-valuemax={MAX_WIDTH}
    tabindex="0"
    onpointerdown={startResize}
    onkeydown={gripKey}
    ondblclick={() => chat.setWidth(400, true)}
    title={m.chat_resize_title()}
  ></div>
  <div class="head">
    <span class="label">{m.chat_title()}</span>
    <div class="acts">
      <button
        class="icon fresh"
        onclick={() => chat.reset()}
        disabled={chat.busy}
        title={m.chat_new()}
        aria-label={m.chat_new()}
      >
        <Icon name="plus-square" size={17} />
      </button>
      <button
        class="icon"
        title={m.chat_providers_title()}
        aria-label={m.chat_provider_settings()}
        onclick={() => chat.openSettings()}
      >
        <Icon name="gear" size={17} />
      </button>
      <button class="icon close" onclick={() => chat.toggle()} title={m.chat_hide_title()} aria-label={m.chat_hide()}>
        <Icon name="x-square" size={17} />
      </button>
    </div>
  </div>

  {#if chat.experiment}
    <div class="trying" class:warn={chat.undoWarning}>
      <div class="trow">
        <span class="tlabel">{m.chat_reply_changed()}</span>
        {#if moved.length}
          <span class="tdelta">
            {#each moved.slice(0, 3) as d}
              <span class={d.pct > 0 ? "up" : "down"}>{signed(d.pct, 1)}% {d.label}</span>
            {/each}
          </span>
        {:else}
          <span class="tdelta dim">{m.chat_nothing_changed()}</span>
        {/if}
        <div class="grow"></div>
        <button class="btn sm" disabled={chat.busy} onclick={() => chat.keepExperiment()}>{m.chat_keep()}</button>
        <button class="btn sm ghost" disabled={chat.busy} onclick={() => chat.undoExperiment()}>{m.chat_undo()}</button>
      </div>
      {#if chat.undoWarning}
        <div class="trow sub">
          <span>{chat.undoWarning}</span>
          <div class="grow"></div>
          <button class="btn sm" disabled={chat.busy} onclick={() => chat.undoExperiment(true)}>{m.chat_undo_anyway()}</button>
          <button class="btn sm ghost" onclick={() => (chat.undoWarning = null)}>{m.common_cancel()}</button>
        </div>
      {/if}
    </div>
  {/if}

  {#if !chat.ready}
    <div class="setup">
      <div class="label">{m.chat_no_provider()}</div>
      <p class="dim">{m.chat_no_provider_blurb()}</p>
      <button class="btn sm" onclick={() => chat.openSettings()}>{m.chat_open_provider_settings()}</button>
    </div>
  {:else}
    <div class="logwrap">
    <div class="log" bind:this={scroller} onscroll={onScroll}>
      {#each chat.turns as turn, i}
        {#if turn.kind === "user"}
          <button
            class="turn user"
            onclick={() => chat.rewindTo(i)}
            disabled={chat.busy}
            title={m.chat_edit_resend()}
          >{turn.text}</button>
        {:else if turn.kind === "assistant"}
          <div class="botwrap">
            <div class="turn bot"><Markdown text={turn.text} /></div>
            <button
              class="copy"
              class:done={copied === i}
              onclick={() => copyText(i, turn.text)}
              title={copied === i ? "Copied" : "Copy this reply"}
              aria-label={m.chat_copy_reply()}
            >
              <Icon name={copied === i ? "check" : "copy"} size={13} />
            </button>
          </div>
        {:else}
          <div class="tool" class:err={turn.status === "error"}>
            <button class="tline" aria-expanded={!!expanded[turn.id]} onclick={() => (expanded[turn.id] = !expanded[turn.id])}>
              <span class="ticon"><Icon name={turn.readOnly ? "eye" : "pencil"} size={11} /></span>
              <span class="tname">{turn.name}</span>
              <span class="targs">{summary(turn)}</span>
              <span class="tstat {turn.status}">{turn.status}</span>
              <span class={["tcaret", { open: expanded[turn.id] }]}><Icon name="caret-right" size={9} /></span>
            </button>
            {#if expanded[turn.id]}
              <div class="tbody">
                <div class="tlabel">{m.chat_tool_args()}</div>
                <pre>{pretty(turn.args ?? {})}</pre>
                {#if turn.result !== undefined}
                  <div class="tlabel">{m.chat_tool_result()}</div>
                  <pre>{pretty(turn.result)}</pre>
                {/if}
              </div>
            {/if}
            {#if turn.status === "awaiting"}
              <div class="approve">
                <span>{m.chat_changes_build()}</span>
                <button class="btn sm" onclick={() => chat.resolveApproval(turn.id, true)}>{m.chat_run()}</button>
                <button class="btn sm ghost" onclick={() => chat.resolveApproval(turn.id, true, true)}>{m.chat_always()}</button>
                <button class="btn sm ghost" onclick={() => chat.resolveApproval(turn.id, false)}>{m.chat_skip()}</button>
                <label class="always"><input type="checkbox" bind:checked={chat.allowWrites} /> {m.chat_allow_all()}</label>
              </div>
            {/if}
            {#if turn.error}<div class="terr">{turn.error}</div>{/if}
          </div>
        {/if}
      {/each}
      {#if chat.busy}
        <div class="dim thinking" aria-live="polite">
          <span class="dots" aria-hidden="true"><i></i><i></i><i></i></span>
          {m.chat_working()}
          {#if chat.startedAt}<WorkingTimer since={chat.startedAt} />{/if}
        </div>
      {/if}
      {#if chat.notice}
        <div class="failed">
          <div class="notice">{chat.notice}</div>
          <div class="failacts">
            {#if chat.canContinue}
              <button class="btn sm ghost" onclick={() => chat.continueRun()} disabled={chat.busy}>{m.chat_continue()}</button>
            {/if}
            <button class="btn sm ghost" onclick={copyDetails} title={m.chat_copy_details_run_title()}>{detailsCopied ? m.common_copied() : m.chat_copy_details()}</button>
          </div>
        </div>
      {/if}
      {#if chat.error}
        <div class="failed">
          <div class="terr">{chat.error}</div>
          <div class="failacts">
            <button class="btn sm ghost" onclick={() => chat.retryLast()} disabled={chat.busy}>{m.chat_try_again()}</button>
            <button class="btn sm ghost" onclick={copyDetails} title={m.chat_copy_details_error_title()}>{detailsCopied ? m.common_copied() : m.chat_copy_details()}</button>
            <button class="btn sm ghost" onclick={() => chat.revealLog().catch((e) => (chat.error = String(e)))} title={m.chat_open_log_title()}>{m.chat_open_log()}</button>
          </div>
        </div>
      {/if}
      {#if !chat.turns.length && !chat.busy}
        <div class="dim empty">
          <div class="egs">
            <button class="eg" onclick={() => { chat.input = m.chat_eg_ehp_prompt(); chat.send(); }}>{m.chat_eg_ehp()}</button>
            <button class="eg" onclick={() => { chat.input = m.chat_eg_defences_prompt(); chat.send(); }}>{m.chat_eg_defences()}</button>
            <button class="eg" onclick={() => { chat.input = m.chat_eg_resists_prompt(); chat.send(); }}>{m.chat_eg_resists()}</button>
          </div>
        </div>
      {/if}
    </div>
    {#if !stuck}
      <button class="toend" onclick={toEnd}><Icon name="arrow-down" size={11} /> {m.chat_scroll_end()}</button>
    {/if}
    </div>

    <div class="composer">
      {#if slashHits.length}
        <div class="slash">
          <div class="slabel">Built-in tools · {chat.toolNames.length} available</div>
          {#each slashHits as t, i}
            <button class="srow" class:sel={i === slashIndex} onclick={() => pickTool(t.name)}>
              <span class="sname">{t.name}</span>
              <span class="skind" class:ro={t.read_only}>{t.read_only ? "read" : "write"}</span>
              <span class="sdesc">{t.description}</span>
            </button>
          {/each}
        </div>
      {/if}
      <textarea
        bind:this={box}
        class="ta"
        rows="2"
        placeholder={m.chat_placeholder()}
        bind:value={chat.input}
        onkeydown={onKeydown}
        onfocus={() => chat.touch()}
        disabled={chat.busy}
      ></textarea>
      <div class="bar">
        <div class="left">
          <ModelPicker />
          {#if chat.supportsEffort || chat.supportsFast}
            <span class="sep"></span>
            <TraitsPicker />
          {/if}
          <span class="sep"></span>
          <ChangesPicker />
        </div>
        <div class="right">
          {#if chat.needsWarm && chat.warmNote}
            <span class="warm {chat.warm}" title={chat.warmDetail || m.chat_warm_title()}>
              <span class="wdot"></span>{chat.warmNote}
            </span>
          {/if}
          <ContextMeter />
          {#if chat.busy}
            <button class="send stop" onclick={() => chat.stop()} title={m.chat_stop()} aria-label={m.chat_stop()}>
              <Icon name="stop" size={11} />
            </button>
          {:else}
            <button class="send" onclick={() => chat.send()} disabled={!chat.input.trim() || chat.warm === "loading" || chat.warm === "priming"} title={chat.warm === "loading" || chat.warm === "priming" ? m.chat_waiting_model() : m.chat_send()} aria-label={m.chat_send()}>
              <Icon name="arrow-up" size={13} />
            </button>
          {/if}
        </div>
      </div>
      {#if chat.modelsError}<div class="terr small">{chat.modelsError}</div>{/if}
    </div>
  {/if}
</aside>

<style>
  .chat {
    position: relative;
    display: flex;
    flex-direction: column;
    flex: none;
    border-left: 1px solid var(--line-1);
    background: var(--bg-1);
    overflow: hidden;
  }
  /* Sits over the border, wider than it looks so it is easy to grab. */
  .grip {
    position: absolute;
    top: 0;
    bottom: 0;
    left: -3px;
    width: 7px;
    z-index: 10;
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
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 8px 10px;
    border-bottom: 1px solid var(--line-1);
  }
  .acts {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .icon {
    display: grid;
    place-items: center;
    width: 28px;
    height: 26px;
    padding: 0;
    background: none;
    border: 1px solid transparent;
    border-radius: var(--r-1);
    color: var(--fg-2);
    cursor: pointer;
  }
  .icon:hover:not(:disabled),
  .icon.on {
    background: var(--bg-hover);
    color: var(--fg-0);
  }
  /* Both hovers reuse existing tokens; no new colours. Red reads as the
     closing action, blue as the one that starts something. */
  .icon.close:hover:not(:disabled) {
    color: var(--bad);
  }
  .icon.fresh:hover:not(:disabled) {
    color: var(--focus);
  }
  .icon:disabled {
    opacity: var(--fade-off);
  }
  .setup {
    padding: 14px;
  }
  .setup p {
    font-size: var(--fs-xs);
    line-height: 1.55;
    margin: 6px 0 12px;
  }
  .logwrap {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .toend {
    position: absolute;
    bottom: 8px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 3px 10px;
    background: var(--bg-2);
    border: 1px solid var(--line-2);
    border-radius: 999px;
    box-shadow: var(--shadow-pop);
    color: var(--fg-1);
    font: inherit;
    font-size: var(--fs-xs);
    cursor: pointer;
  }
  .toend:hover {
    color: var(--fg-0);
  }
  .log {
    flex: 1;
    overflow-y: auto;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .turn {
    font-size: var(--fs-md);
    line-height: 1.55;
    white-space: pre-wrap;
    word-break: break-word;
  }
  /* A button so it is reachable by keyboard, styled as the message bubble. */
  .user {
    align-self: flex-end;
    max-width: 88%;
    background: var(--bg-3);
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    padding: 6px 9px;
    font: inherit;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .user:hover:not(:disabled) {
    background: var(--bg-active);
  }
  .user:focus-visible {
    border-color: var(--focus);
    outline: none;
  }
  .user:disabled {
    cursor: default;
  }
  .failed {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .failed .terr,
  .failed .notice {
    flex: 1 1 100%;
  }
  .failacts {
    display: flex;
    gap: 6px;
    margin-left: auto;
  }
  /* The button overlaps the text, so it only appears on hover or focus. */
  .botwrap {
    position: relative;
  }
  .bot {
    color: var(--fg-0);
    white-space: normal;
  }
  .copy {
    position: absolute;
    top: -2px;
    right: 0;
    display: grid;
    place-items: center;
    width: 22px;
    height: 20px;
    padding: 0;
    background: var(--bg-2);
    border: 1px solid var(--line-1);
    border-radius: var(--r-1);
    color: var(--fg-3);
    /* Faint rather than hidden: a hover-only control goes unfound. */
    opacity: 0.3;
    transition: opacity 90ms linear;
    cursor: pointer;
  }
  .botwrap:hover .copy,
  .copy:focus-visible,
  .copy.done {
    opacity: 1;
  }
  .copy:hover {
    color: var(--fg-0);
  }
  .copy.done {
    color: var(--ok);
  }
  .tool {
    border-left: 2px solid var(--line-2);
    padding: 3px 0 3px 8px;
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
  }
  .tool.err {
    border-left-color: var(--bad);
  }
  .tline {
    display: flex;
    gap: 6px;
    align-items: center;
    width: 100%;
    padding: 0;
    background: none;
    border: 0;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .tline:hover .tname {
    color: var(--fg-0);
  }
  .ticon,
  .tcaret {
    display: grid;
    place-items: center;
    color: var(--fg-3);
    flex: none;
  }
  .tcaret {
    transition: transform 120ms ease;
  }
  .tcaret.open {
    transform: rotate(90deg);
  }
  .tbody {
    margin: 4px 0 2px 17px;
  }
  .tlabel {
    color: var(--fg-3);
    font-family: var(--font-ui);
    font-size: var(--fs-2xs);
    margin: 4px 0 2px;
  }
  .tbody pre {
    margin: 0;
    max-height: 240px;
    overflow: auto;
    padding: 6px 8px;
    background: var(--bg-2);
    border: 1px solid var(--line-1);
    border-radius: var(--r-1);
    color: var(--fg-1);
    font-size: var(--fs-2xs);
    white-space: pre-wrap;
    word-break: break-word;
  }
  .tname {
    color: var(--fg-1);
  }
  .targs {
    color: var(--fg-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
  }
  .tstat {
    color: var(--fg-3);
    font-size: var(--fs-2xs);
  }
  .tstat.done {
    color: var(--ok);
  }
  .tstat.error {
    color: var(--bad);
  }
  .tstat.awaiting {
    color: var(--warn);
  }
  .approve {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 5px;
    color: var(--warn);
    font-size: var(--fs-2xs);
  }
  .always {
    display: flex;
    align-items: center;
    gap: 3px;
    color: var(--fg-3);
  }
  .terr {
    color: var(--bad);
    font-size: var(--fs-xs);
    word-break: break-word;
  }
  .notice {
    color: var(--warn);
    font-size: var(--fs-xs);
    line-height: 1.45;
  }
  .thinking {
    font-size: var(--fs-xs);
    display: flex;
    align-items: center;
    gap: 6px;
  }
  /* Three dots pulsing in sequence, in the text colour: the same beat as the
     status bar pulse so the two indicators read as one system. */
  .dots {
    display: inline-flex;
    gap: 3px;
    margin-left: 4px;
  }
  .dots i {
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: currentColor;
    animation: pulse 1.2s ease-in-out infinite;
  }
  .dots i:nth-child(2) {
    animation-delay: 0.2s;
  }
  .dots i:nth-child(3) {
    animation-delay: 0.4s;
  }
  .empty {
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  .egs {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-top: 10px;
  }
  .eg {
    text-align: left;
    background: none;
    border: 1px solid var(--line-1);
    border-radius: var(--r-1);
    color: var(--fg-2);
    font: inherit;
    font-size: var(--fs-xs);
    padding: 4px 7px;
    cursor: pointer;
  }
  .eg:hover {
    background: var(--bg-hover);
    color: var(--fg-0);
  }

  /* Composer: one bordered card holding the box and its controls. */
  .composer {
    container: composer / inline-size;
    position: relative;
    margin: 8px;
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    background: var(--bg-2);
  }
  .composer:focus-within {
    border-color: var(--line-2);
  }
  .ta {
    display: block;
    width: 100%;
    box-sizing: border-box;
    resize: none;
    background: none;
    border: 0;
    outline: none;
    color: var(--fg-0);
    font-family: var(--font-ui);
    font-size: var(--fs-md);
    line-height: 1.5;
    padding: 9px 10px 4px;
    field-sizing: content;
    min-height: calc(3em + 13px);
    max-height: 200px;
  }
  .ta::placeholder {
    color: var(--fg-3);
  }
  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    padding: 4px 6px 6px 4px;
  }
  .left {
    display: flex;
    align-items: center;
    gap: 1px;
    min-width: 0;
    flex: 1;
  }
  .right {
    display: flex;
    align-items: center;
    gap: 4px;
    flex: none;
  }
  .sep {
    width: 1px;
    height: 14px;
    margin: 0 3px;
    background: var(--line-1);
    flex: none;
  }
  .grow {
    flex: 1;
  }
  .warm {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: var(--fs-2xs);
    color: var(--fg-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 200px;
  }
  .wdot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--fg-3);
    flex: 0 0 auto;
  }
  .warm.loading .wdot,
  .warm.priming .wdot {
    background: var(--warn);
    animation: pulse 1.2s ease-in-out infinite;
  }
  .warm.ready .wdot {
    background: var(--ok);
  }
  .warm.failed .wdot {
    background: var(--bad);
  }
  .trying {
    border-bottom: 1px solid var(--line-1);
    background: var(--bg-2);
    font-size: var(--fs-2xs);
  }
  .trying.warn {
    border-left: 2px solid var(--warn);
  }
  .trow {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 8px;
  }
  .trow.sub {
    border-top: 1px solid var(--line-1);
    color: var(--warn);
  }
  .tlabel {
    color: var(--fg-2);
  }
  .tdelta {
    display: flex;
    gap: 7px;
    font-family: var(--font-mono);
  }
  .tdelta .up {
    color: var(--ok);
  }
  .tdelta .down {
    color: var(--bad);
  }
  .tdelta.dim {
    color: var(--fg-3);
  }
  .send {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    padding: 0;
    background: var(--fg-0);
    border: 0;
    border-radius: 50%;
    color: var(--bg-1);
    cursor: pointer;
    transition: transform 100ms ease;
  }
  .send:hover:not(:disabled) {
    transform: scale(1.06);
  }
  .send:disabled {
    background: var(--bg-3);
    color: var(--fg-4);
    cursor: default;
  }
  .send.stop {
    background: var(--bg-3);
    border: 1px solid var(--line-2);
    color: var(--fg-0);
  }

  .slash {
    position: absolute;
    bottom: calc(100% + 6px);
    left: 0;
    right: 0;
    max-height: 260px;
    overflow-y: auto;
    background: var(--bg-2);
    border: 1px solid var(--line-2);
    border-radius: var(--r-2);
    padding: 4px;
    z-index: 5;
  }
  .slabel {
    color: var(--fg-3);
    font-size: var(--fs-2xs);
    padding: 3px 6px 5px;
  }
  .srow {
    display: block;
    width: 100%;
    text-align: left;
    background: none;
    border: 0;
    border-radius: var(--r-1);
    padding: 4px 6px;
    color: var(--fg-1);
    font: inherit;
    cursor: pointer;
  }
  .srow:hover,
  .srow.sel {
    background: var(--bg-hover);
  }
  .sname {
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    color: var(--fg-0);
  }
  .skind {
    font-family: var(--font-mono);
    font-size: var(--fs-2xs);
    color: var(--warn);
    margin-left: 6px;
  }
  .skind.ro {
    color: var(--fg-3);
  }
  .sdesc {
    display: block;
    color: var(--fg-3);
    font-size: var(--fs-2xs);
    line-height: 1.4;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
