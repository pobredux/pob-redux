<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { handsfree, type SetupEvent } from "$lib/state/handsfree.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { m } from "$lib/paraglide/messages";

  let { onclose }: { onclose: () => void } = $props();

  const HISTORY = 160;
  let started = $state(false);
  let busy = $state(false);
  let levels = $state<number[]>([]);
  let waveWidth = $state(0);

  onMount(() => {
    const off = listen<number>("wake:level", (e) => (levels = [...levels.slice(1 - HISTORY), e.payload]));
    return () => void off.then((f) => f());
  });

  type Of<S extends SetupEvent["stage"]> = Extract<SetupEvent, { stage: S }>;
  const events = $derived(handsfree.setup);
  function lastOf<T extends SetupEvent>(test: (e: SetupEvent) => e is T): T | undefined {
    for (let i = events.length - 1; i >= 0; i--) {
      const e = events[i];
      if (test(e)) return e;
    }
    return undefined;
  }
  const last = <S extends SetupEvent["stage"]>(stage: S) => lastOf((e): e is Of<S> => e.stage === stage);
  const asks = $derived(events.filter((e) => e.stage === "ask").length);
  const results = $derived(events.filter((e): e is Of<"result"> => e.stage === "result"));
  const turnEvent = $derived(lastOf((e): e is Of<"ask"> | Of<"result"> => e.stage === "ask" || e.stage === "result"));
  const reading = $derived(last("reading"));
  const read = $derived(last("read"));
  const fitted = $derived(last("fitted"));
  const done = $derived(last("done"));
  const failed = $derived(last("failed"));
  const step = $derived(failed ? "failed" : done ? "done" : fitted ? "test" : read ? "read" : started ? "collect" : "intro");
  const number = $derived(step === "collect" ? 1 : step === "read" ? 2 : step === "test" ? 3 : 0);
  const collected = $derived(results.filter((r) => !r.test && r.voice).length);
  const total = $derived(step === "test" ? 3 : (turnEvent?.total ?? 8));
  const tested = $derived(results.filter((r) => r.test && r.voice));
  const turn = $derived.by(() => {
    if (!turnEvent) return "wait";
    if (turnEvent.stage === "ask") return "speak";
    if (!turnEvent.voice) return "again";
    if (!turnEvent.test) return "got";
    return turnEvent.personal ? "got" : "missed";
  });
  const readShare = $derived(reading ? Math.min(1, reading.voiced / reading.needed) : 0);
  const bars = $derived.by(() => {
    const n = Math.min(HISTORY, Math.max(8, Math.floor((waveWidth - 22) / 4)));
    return Array.from({ length: n }, (_, i) => 2 + Math.min(1, Math.sqrt(levels[levels.length - n + i] ?? 0) * 3) * 20);
  });

  let cued = 0;
  $effect(() => {
    if (asks > cued) {
      cued = asks;
      tick();
    }
  });

  function tick() {
    try {
      const ctx = new AudioContext();
      const gain = ctx.createGain();
      gain.gain.setValueAtTime(0.06, ctx.currentTime);
      gain.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + 0.12);
      gain.connect(ctx.destination);
      const o = ctx.createOscillator();
      o.frequency.value = 740;
      o.connect(gain);
      o.start();
      o.stop(ctx.currentTime + 0.12);
      setTimeout(() => void ctx.close(), 400);
    } catch {}
  }

  async function start() {
    busy = true;
    levels = [];
    cued = 0;
    try {
      await handsfree.startSetup();
      started = true;
    } catch (e) {
      handsfree.error = String(e);
    } finally {
      busy = false;
    }
  }

  async function close(save: boolean) {
    busy = true;
    try {
      if (save) await handsfree.saveSetup();
      else await handsfree.cancelSetup();
    } catch (e) {
      handsfree.error = String(e);
    } finally {
      busy = false;
      onclose();
    }
  }
</script>

<div class="overlay" role="presentation">
  <div
    class="modal"
    role="dialog"
    aria-modal="true"
    aria-label={m.wake_setup_title()}
    tabindex="-1"
    onkeydown={(e) => {
      if (e.key === "Escape" && !busy) void close(false);
    }}
  >
    <div class="mhead">
      <span class="label">{m.wake_setup_title()}</span>
      <button class="icon" aria-label={m.common_cancel()} disabled={busy} onclick={() => close(false)}><Icon name="x" size={14} /></button>
    </div>
    <div class="body">
      <div class="steps" aria-hidden={number === 0}>
        {#each [1, 2, 3] as n}
          <span class="pip" class:on={n === number} class:past={n < number || step === "done"}></span>
        {/each}
        {#if number}<span class="steplabel">{m.wake_setup_step({ step: number })}</span>{/if}
      </div>

      {#if step === "intro"}
        <p class="lead">{m.wake_setup_intro()}</p>
      {:else if step === "collect" || step === "test"}
        <div class="turnrow">
          <div class="turn {turn}" aria-hidden="true">
            <Icon name={turn === "got" ? "check" : turn === "again" || turn === "missed" ? "x" : "microphone"} size={26} />
          </div>
          <div class="turntext" aria-live="polite">
            <p class="lead">
              {#if turn === "speak"}
                {step === "test" ? m.wake_setup_test() : m.wake_setup_say()}
              {:else if turn === "got"}
                {step === "test" ? m.wake_setup_recognised() : m.wake_setup_got_it()}
              {:else if turn === "missed"}
                {m.wake_setup_not_recognised()}
              {:else if turn === "again"}
                {m.wake_setup_again()}
              {:else}
                {m.wake_setup_ready()}
              {/if}
            </p>
            <p class="sub">{m.wake_setup_count({ done: step === "test" ? tested.length : collected, total })}</p>
          </div>
        </div>
        <div class="dots" aria-hidden="true">
          {#each Array.from({ length: total }) as _, i}
            {@const r = step === "test" ? tested[i] : undefined}
            <span class="dot" class:filled={step === "test" ? !!r : i < collected} class:miss={r && !r.personal}></span>
          {/each}
        </div>
      {:else if step === "read"}
        <p class="lead">{m.wake_setup_read()}</p>
        <blockquote>{m.wake_setup_paragraph()}</blockquote>
        <div class="progress" aria-hidden="true"><span style:width="{readShare * 100}%"></span></div>
        <p class="sub" aria-live="polite">{readShare >= 1 ? m.wake_setup_read_enough() : m.wake_setup_read_more()}</p>
      {:else if step === "done" && done}
        <div class="turnrow">
          <div class="turn got" aria-hidden="true"><Icon name="check" size={26} /></div>
          <p class="lead">{m.wake_setup_done({ personal: done.personal, tries: done.tries })}</p>
        </div>
        {#if fitted && !fitted.separated}<p class="warnline">{m.wake_setup_overlap()}</p>{/if}
      {:else if step === "failed" && failed}
        <div class="turnrow">
          <div class="turn again" aria-hidden="true"><Icon name="x" size={26} /></div>
          <p class="lead">{failed.error}</p>
        </div>
      {/if}

      <div class="wave" class:idle={step === "intro" || step === "done" || step === "failed"} aria-hidden="true" bind:clientWidth={waveWidth}>
        {#each bars as h, i (i)}<i style:height="{h}px"></i>{/each}
      </div>
      {#if handsfree.error}<p class="warnline mono">{handsfree.error}</p>{/if}
    </div>
    <div class="foot">
      {#if step === "intro"}
        <button class="btn sm ghost" disabled={busy} onclick={() => close(false)}>{m.common_cancel()}</button>
        <button class="btn sm primary" disabled={busy} onclick={start}>{m.wake_setup_start()}</button>
      {:else if step === "read"}
        <button class="btn sm ghost" disabled={busy} onclick={() => close(false)}>{m.common_cancel()}</button>
        <button class="btn sm" disabled={busy || readShare < 0.5} onclick={() => handsfree.nextSetup()}>{m.wake_setup_done_reading()}</button>
      {:else if step === "done"}
        <button class="btn sm ghost" disabled={busy} onclick={() => close(false)}>{m.wake_setup_discard()}</button>
        <button class="btn sm primary" disabled={busy} onclick={() => close(true)}>{m.wake_setup_save()}</button>
      {:else if step === "failed"}
        <button class="btn sm ghost" disabled={busy} onclick={() => close(false)}>{m.common_cancel()}</button>
        <button class="btn sm primary" disabled={busy} onclick={start}>{m.wake_setup_retry()}</button>
      {:else}
        <button class="btn sm ghost" disabled={busy} onclick={() => close(false)}>{m.common_cancel()}</button>
      {/if}
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 120;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--backdrop);
  }
  .modal {
    width: 500px;
    max-width: 92vw;
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    background: var(--bg-1);
    box-shadow: var(--shadow-modal);
  }
  .mhead {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 8px 8px 12px;
    border-bottom: 1px solid var(--line-0);
  }
  .icon {
    display: grid;
    place-items: center;
    width: 26px;
    height: 24px;
    padding: 0;
    border: 0;
    border-radius: var(--r-1);
    background: none;
    color: var(--fg-2);
  }
  .icon:hover:not(:disabled) {
    background: var(--bg-hover);
    color: var(--fg-0);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 14px;
    height: 340px;
    padding: 14px 16px 16px;
    color: var(--fg-1);
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  p {
    margin: 0;
  }
  .steps {
    display: flex;
    flex: none;
    align-items: center;
    gap: 6px;
    height: 16px;
  }
  .pip {
    width: 18px;
    height: 3px;
    border-radius: 2px;
    background: var(--line-1);
  }
  .pip.past {
    background: var(--fg-3);
  }
  .pip.on {
    background: var(--fg-0);
  }
  .steplabel {
    margin-left: 6px;
    color: var(--fg-3);
    font-size: var(--fs-xs);
  }
  .lead {
    color: var(--fg-0);
    font-size: var(--fs-lg);
    line-height: 1.4;
  }
  .sub {
    color: var(--fg-2);
  }
  .turnrow {
    display: flex;
    align-items: center;
    gap: 18px;
    min-height: 88px;
  }
  .turntext {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .turn {
    position: relative;
    display: grid;
    flex: none;
    place-items: center;
    width: 72px;
    height: 72px;
    border: 2px solid var(--line-2);
    border-radius: 50%;
    color: var(--fg-3);
    transition:
      border-color 120ms linear,
      color 120ms linear;
  }
  .turn.speak {
    border-color: var(--ok);
    color: var(--ok);
  }
  .turn.speak::after {
    content: "";
    position: absolute;
    inset: -2px;
    border: 2px solid var(--ok);
    border-radius: 50%;
    animation: ring 1.2s ease-out infinite;
  }
  .turn.got {
    border-color: var(--ok);
    background: color-mix(in oklab, var(--ok) 14%, transparent);
    color: var(--ok);
  }
  .turn.again,
  .turn.missed {
    border-color: var(--warn);
    color: var(--warn);
  }
  @keyframes ring {
    from {
      transform: scale(1);
      opacity: 0.8;
    }
    to {
      transform: scale(1.35);
      opacity: 0;
    }
  }
  .dots {
    display: flex;
    gap: 6px;
  }
  .dot {
    width: 10px;
    height: 10px;
    border: 1px solid var(--line-2);
    border-radius: 50%;
  }
  .dot.filled {
    border-color: var(--ok);
    background: var(--ok);
  }
  .dot.filled.miss {
    border-color: var(--warn);
    background: var(--warn);
  }
  blockquote {
    margin: 0;
    padding: 10px 12px;
    border-left: 2px solid var(--line-2);
    background: var(--bg-2);
    color: var(--fg-0);
    font-size: var(--fs-md);
    line-height: 1.55;
  }
  .progress {
    height: 4px;
    overflow: hidden;
    border-radius: 2px;
    background: var(--line-1);
  }
  .progress span {
    display: block;
    height: 100%;
    background: var(--ok);
    transition: width 300ms linear;
  }
  .warnline {
    color: var(--warn);
  }
  .wave {
    display: flex;
    flex: none;
    align-items: center;
    justify-content: flex-end;
    gap: 2px;
    height: 30px;
    margin-top: auto;
    padding: 0 12px;
    overflow: hidden;
    border-radius: 999px;
    background: var(--bg-0);
  }
  .wave.idle {
    visibility: hidden;
  }
  .wave i {
    flex: none;
    width: 2px;
    border-radius: 1px;
    background: var(--fg-0);
    transition: height 70ms linear;
  }
  .foot {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 8px 12px 10px;
    border-top: 1px solid var(--line-0);
  }
</style>
