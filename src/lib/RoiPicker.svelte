<script lang="ts">
  // Alegerea zonei analizate: o captură din joc + un dreptunghi care se mută și se redimensionează.
  import type { Roi } from "./api";
  let { roi, image, onchange }: { roi: Roi; image: ImageData | null; onchange: (r: Roi) => void } = $props();

  let box: HTMLDivElement;
  let canvas = $state<HTMLCanvasElement>();
  let r = $state<Roi>({ x0: 0, y0: 0, x1: 0, y1: 0 });
  let drag: { mode: string; sx: number; sy: number; start: Roi } | null = null;

  $effect(() => {
    if (!drag) r = { ...roi };
  });

  $effect(() => {
    if (image && canvas) {
      canvas.width = image.width;
      canvas.height = image.height;
      canvas.getContext("2d")!.putImageData(image, 0, 0);
    }
  });

  function down(e: PointerEvent, mode: string) {
    e.stopPropagation();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag = { mode, sx: e.clientX, sy: e.clientY, start: { ...r } };
  }
  function move(e: PointerEvent) {
    if (!drag) return;
    const b = box.getBoundingClientRect();
    const dx = (e.clientX - drag.sx) / b.width;
    const dy = (e.clientY - drag.sy) / b.height;
    const s = drag.start;
    const c = (v: number) => Math.max(0, Math.min(1, v));
    const n = { ...s };
    if (drag.mode === "move") {
      const w = s.x1 - s.x0, h = s.y1 - s.y0;
      n.x0 = c(Math.min(s.x0 + dx, 1 - w)); n.x1 = n.x0 + w;
      n.y0 = c(Math.min(s.y0 + dy, 1 - h)); n.y1 = n.y0 + h;
    } else {
      if (drag.mode.includes("w")) n.x0 = c(Math.min(s.x0 + dx, s.x1 - 0.04));
      if (drag.mode.includes("e")) n.x1 = c(Math.max(s.x1 + dx, s.x0 + 0.04));
      if (drag.mode.includes("n")) n.y0 = c(Math.min(s.y0 + dy, s.y1 - 0.04));
      if (drag.mode.includes("s")) n.y1 = c(Math.max(s.y1 + dy, s.y0 + 0.04));
    }
    r = n;
  }
  function up() {
    if (drag) {
      drag = null;
      onchange({ ...r });
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="picker" bind:this={box} onpointermove={move} onpointerup={up}>
  {#if image}
    <canvas bind:this={canvas}></canvas>
  {:else}
    <div class="ph">Fă o captură ca să vezi ecranul jocului aici</div>
  {/if}
  <div class="shade" style="clip-path: polygon(0 0,100% 0,100% 100%,0 100%,0 {r.y0 * 100}%,{r.x0 * 100}% {r.y0 * 100}%,{r.x0 * 100}% {r.y1 * 100}%,{r.x1 * 100}% {r.y1 * 100}%,{r.x1 * 100}% {r.y0 * 100}%,0 {r.y0 * 100}%)"></div>
  <div
    class="rect"
    style="left:{r.x0 * 100}%;top:{r.y0 * 100}%;width:{(r.x1 - r.x0) * 100}%;height:{(r.y1 - r.y0) * 100}%"
    onpointerdown={(e) => down(e, "move")}
  >
    <span class="tag">Zona promptului</span>
    {#each ["nw", "ne", "sw", "se"] as h}
      <span class="h {h}" onpointerdown={(e) => down(e, h)}></span>
    {/each}
  </div>
  <div class="cross"></div>
</div>

<style>
  .picker { position: relative; width: 100%; aspect-ratio: 16 / 9; border-radius: 14px; overflow: hidden; background: #0d0806; border: 2.5px solid var(--ink); touch-action: none; }
  canvas { width: 100%; height: 100%; display: block; object-fit: cover; }
  .ph { position: absolute; inset: 0; display: grid; place-items: center; color: var(--dim); font-size: 13px; background: repeating-linear-gradient(45deg, #1a110c 0 14px, #160e0a 14px 28px); }
  .shade { position: absolute; inset: 0; background: rgba(10, 6, 4, 0.55); pointer-events: none; }
  .rect { position: absolute; border: 3px solid var(--gold); border-radius: 8px; cursor: move; box-shadow: 0 0 0 2px var(--ink), inset 0 0 0 2px var(--ink); }
  .tag { position: absolute; top: -28px; left: -3px; padding: 2px 9px; border-radius: 8px; background: var(--gold); color: var(--ink); border: 2px solid var(--ink); font-size: 11.5px; font-weight: 700; white-space: nowrap; }
  .h { position: absolute; width: 16px; height: 16px; border-radius: 5px; background: #fff3c4; border: 2.5px solid var(--ink); }
  .nw { left: -9px; top: -9px; cursor: nwse-resize; }
  .ne { right: -9px; top: -9px; cursor: nesw-resize; }
  .sw { left: -9px; bottom: -9px; cursor: nesw-resize; }
  .se { right: -9px; bottom: -9px; cursor: nwse-resize; }
  .cross { position: absolute; left: 50%; top: 50%; width: 14px; height: 14px; margin: -7px; pointer-events: none;
    background: linear-gradient(#fff, #fff) center/2px 100% no-repeat, linear-gradient(#fff, #fff) center/100% 2px no-repeat; opacity: 0.7; }
</style>
