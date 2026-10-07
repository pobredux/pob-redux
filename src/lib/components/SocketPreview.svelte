<script lang="ts">
  import { m } from "$lib/paraglide/messages";
  import { build } from "$lib/state/build.svelte";
  import { loadTree } from "$lib/tree/load";
  import type { TreeModel } from "$lib/tree/model";

  let { socket, radius, label }: { socket: number; radius: number; label: string } = $props();

  const SIZE = 240;
  let model = $state<TreeModel | null>(null);
  let canvas = $state<HTMLCanvasElement | null>(null);

  $effect(() => {
    const v = build.tree?.treeVersion;
    if (!v) return;
    let live = true;
    loadTree(v)
      .then((t) => live && (model = t.model))
      .catch(() => {});
    return () => {
      live = false;
    };
  });

  const allocated = $derived(new Set(build.tree?.allocatedNodes ?? []));

  const takenInRange = $derived.by(() => {
    const centre = model?.nodes.get(socket);
    if (!model || !centre) return 0;
    let count = 0;
    for (const id of allocated) {
      const n = model.nodes.get(id);
      if (n && n.id !== socket && !n.asc && (n.x - centre.x) ** 2 + (n.y - centre.y) ** 2 <= radius * radius) count++;
    }
    return count;
  });

  $effect(() => {
    if (canvas && model) draw(canvas, model, socket, radius, allocated);
  });

  function draw(c: HTMLCanvasElement, M: TreeModel, socketId: number, r: number, alloc: Set<number>) {
    const dpr = window.devicePixelRatio || 1;
    c.width = SIZE * dpr;
    c.height = SIZE * dpr;
    const g = c.getContext("2d");
    if (!g) return;
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.clearRect(0, 0, SIZE, SIZE);
    const centre = M.nodes.get(socketId);
    if (!centre) return;
    const css = getComputedStyle(c);
    const tok = (name: string) => css.getPropertyValue(name).trim();
    const half = r * 1.35;
    const k = SIZE / (2 * half);
    const sx = (x: number) => (x - centre.x) * k + SIZE / 2;
    const sy = (y: number) => (y - centre.y) * k + SIZE / 2;
    const near = (x: number, y: number) => Math.abs(x - centre.x) <= half && Math.abs(y - centre.y) <= half;
    const inRange = (x: number, y: number) => (x - centre.x) ** 2 + (y - centre.y) ** 2 <= r * r;

    g.lineWidth = 1;
    for (const e of M.edges) {
      const [x0, y0, x1, y1] = e.box;
      if (e.asc || x1 < centre.x - half || x0 > centre.x + half || y1 < centre.y - half || y0 > centre.y + half) continue;
      const a = M.nodes.get(e.a);
      const b = M.nodes.get(e.b);
      if (!a || !b || a.hidden || b.hidden) continue;
      g.strokeStyle = alloc.has(e.a) && alloc.has(e.b) ? tok("--fg-2") : tok("--line-2");
      g.beginPath();
      if (e.arc) g.arc(sx(e.arc.cx), sy(e.arc.cy), e.arc.r * k, e.arc.a1, e.arc.a2, e.arc.ccw);
      else {
        g.moveTo(sx(a.x), sy(a.y));
        g.lineTo(sx(b.x), sy(b.y));
      }
      g.stroke();
    }

    const keystones: { x: number; y: number; name: string }[] = [];
    for (const n of M.nodes.values()) {
      if (n.asc || n.hidden || n.id === socketId || !near(n.x, n.y)) continue;
      if (n.kind === "classStart" || n.kind === "ascStart" || n.kind === "onlyImage" || n.kind === "mastery") continue;
      const size = n.kind === "keystone" ? 5 : n.kind === "notable" ? 3.5 : n.kind === "socket" ? 3.5 : 2;
      g.fillStyle = alloc.has(n.id) ? tok("--fg-0") : inRange(n.x, n.y) ? tok("--fg-3") : tok("--fg-4");
      g.beginPath();
      g.arc(sx(n.x), sy(n.y), size, 0, Math.PI * 2);
      g.fill();
      if (n.kind === "keystone") keystones.push({ x: sx(n.x), y: sy(n.y), name: n.name });
    }

    g.strokeStyle = tok("--focus");
    g.lineWidth = 1.5;
    g.beginPath();
    g.arc(SIZE / 2, SIZE / 2, r * k, 0, Math.PI * 2);
    g.stroke();
    g.fillStyle = tok("--focus");
    g.beginPath();
    g.arc(SIZE / 2, SIZE / 2, 4.5, 0, Math.PI * 2);
    g.fill();

    g.font = `10px ${css.fontFamily}`;
    g.textAlign = "center";
    g.fillStyle = tok("--fg-1");
    for (const ks of keystones) g.fillText(ks.name, Math.min(Math.max(ks.x, 40), SIZE - 40), ks.y - 8);
  }
</script>

<div class="preview">
  <canvas bind:this={canvas} style:width={`${SIZE}px`} style:height={`${SIZE}px`}></canvas>
  <div class="cap">
    <span class="name">{label}</span>
    <span class="dim">{m.timeless_preview_taken()} <span class="num">{takenInRange}</span></span>
  </div>
</div>

<style>
  .preview {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  canvas {
    display: block;
    background: var(--bg-1);
    border-radius: var(--r-1);
  }
  .cap {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: var(--fs-xs);
  }
  .name {
    color: var(--fg-0);
  }
  .dim {
    color: var(--fg-3);
  }
  .num {
    color: var(--fg-1);
    font-family: var(--font-mono);
  }
</style>
