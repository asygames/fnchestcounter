<script lang="ts">
  // Grafic cu bare pe zile. Barele cresc animat (transform), iar la hover arată valoarea.
  let { data }: { data: { key: string; label: string; value: number; today: boolean }[] } = $props();

  const max = $derived(Math.max(5, ...data.map((d) => d.value)));
  const nice = $derived.by(() => {
    const step = Math.pow(10, Math.floor(Math.log10(max)));
    const s = max / step > 5 ? step * 2 : max / step > 2 ? step : step / 2;
    return Math.ceil(max / s) * s;
  });
  const ticks = $derived([0, 0.25, 0.5, 0.75, 1].map((f) => Math.round(nice * f)));
  let hover = $state<number | null>(null);
  let mounted = $state(false);
  $effect(() => {
    const t = setTimeout(() => (mounted = true), 30);
    return () => clearTimeout(t);
  });
  const showEvery = $derived(data.length > 45 ? 10 : data.length > 20 ? 5 : 1);
</script>

<div class="chart">
  <div class="axis">
    {#each ticks.slice().reverse() as t}<span>{t}</span>{/each}
  </div>
  <div class="plot">
    {#each ticks as _, i}<div class="grid" style="bottom:{(i / 4) * 100}%"></div>{/each}
    <div class="bars" style="--n:{data.length}">
      {#each data as d, i (d.key)}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="slot" onmouseenter={() => (hover = i)} onmouseleave={() => (hover = null)}>
          <div
            class="bar"
            class:today={d.today}
            class:zero={d.value === 0}
            style="transform:scaleY({mounted ? Math.max(d.value === 0 ? 0 : 0.02, d.value / nice) : 0});transition-delay:{i * 12}ms"
          ></div>
          {#if hover === i}
            <div class="tip" style="bottom:{(d.value / nice) * 100}%">
              <b class="num">{d.value}</b> <span>{d.label}</span>
            </div>
          {/if}
          {#if i % showEvery === 0 || i === data.length - 1}
            <span class="xl">{d.label.split(" ")[0]}</span>
          {/if}
        </div>
      {/each}
    </div>
  </div>
</div>

<style>
  .chart { display: grid; grid-template-columns: 34px 1fr; gap: 8px; height: 100%; min-height: 220px; }
  .axis { display: flex; flex-direction: column; justify-content: space-between; text-align: right; font-size: 11px; color: var(--dim); padding-bottom: 22px; margin-top: -6px; }
  .plot { position: relative; padding-bottom: 22px; }
  .grid { position: absolute; left: 0; right: 0; height: 0; border-top: 1.5px dashed rgba(143, 108, 83, 0.25); margin-bottom: 22px; }
  .bars { position: absolute; inset: 0 0 22px 0; display: grid; grid-template-columns: repeat(var(--n), 1fr); gap: 3px; align-items: end; }
  .slot { position: relative; height: 100%; display: flex; align-items: flex-end; justify-content: center; }
  .bar {
    width: 100%; max-width: 30px; height: 100%;
    transform-origin: bottom;
    background: linear-gradient(180deg, #ffb35a, var(--orange) 60%, #d45e0c);
    border: 2px solid var(--ink);
    border-radius: 7px 7px 3px 3px;
    box-shadow: inset 0 2px 0 rgba(255, 255, 255, 0.35);
    transition: transform 0.6s cubic-bezier(0.2, 1.2, 0.4, 1), filter 0.15s;
  }
  .bar.today { background: linear-gradient(180deg, #fff3b0, var(--gold) 55%, #e08a0c); }
  .bar.zero { border-width: 0; }
  .slot:hover .bar { filter: brightness(1.15); }
  .tip {
    position: absolute; transform: translateY(-10px); white-space: nowrap; z-index: 3;
    background: var(--text); color: var(--ink); border: 2px solid var(--ink); border-radius: 10px;
    padding: 4px 9px; font-size: 12px; box-shadow: 0 3px 0 var(--ink); pointer-events: none;
  }
  .tip b { font-size: 15px; }
  .xl { position: absolute; bottom: -20px; font-size: 10.5px; color: var(--dim); white-space: nowrap; }
</style>
