<script lang="ts">
  let { since }: { since: number } = $props();

  let now = $state(Date.now());
  const seconds = $derived(Math.max(0, Math.floor((now - since) / 1000)));
  const text = $derived(seconds < 60 ? `${seconds}s` : `${Math.floor(seconds / 60)}m ${String(seconds % 60).padStart(2, "0")}s`);
</script>

<span class="t" {@attach () => {
  const id = setInterval(() => (now = Date.now()), 1000);
  return () => clearInterval(id);
}}>{text}</span>

<style>
  .t {
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }
</style>
