<script lang="ts">
  import { app, duration, fmt } from "../lib/state.svelte";
  import BarChart from "../lib/BarChart.svelte";
  import Icon from "../lib/Icon.svelte";

  const s = $derived(app.snap!);
  let range = $state(30);

  const dayMap = $derived(new Map(s.days));
  const series = $derived.by(() => {
    const out: { key: string; label: string; value: number; today: boolean }[] = [];
    const d = new Date();
    for (let i = range - 1; i >= 0; i--) {
      const x = new Date(d);
      x.setDate(d.getDate() - i);
      const key = x.toLocaleDateString("sv-SE");
      out.push({
        key,
        label: x.toLocaleDateString("ro-RO", { day: "numeric", month: "short" }),
        value: dayMap.get(key) ?? 0,
        today: i === 0,
      });
    }
    return out;
  });
  const inRange = $derived(series.reduce((a, b) => a + b.value, 0));
  const active = $derived(series.filter((x) => x.value > 0).length);
  const avg = $derived(active ? inRange / active : 0);
  const best = $derived(series.reduce((m, x) => (x.value > m.value ? x : m), { key: "", label: "—", value: 0, today: false }));
  const streak = $derived.by(() => {
    let n = 0;
    for (let i = series.length - 1; i >= 0; i--) {
      if (series[i].value > 0) n++;
      else if (i === series.length - 1) continue; // azi încă n-ai jucat: nu rupe seria
      else break;
    }
    return n;
  });
</script>

<div class="top">
  <div>
    <h1>Statistici</h1>
    <p class="muted">Cuferele tale pe zile și pe sesiuni. Totul e salvat local, pe calculatorul tău.</p>
  </div>
  <div class="seg">
    {#each [7, 30, 90] as r}
      <button class:on={range === r} onclick={() => (range = r)}>{r} zile</button>
    {/each}
  </div>
</div>

<section class="tiles">
  <div class="tile card"><div class="label">În {range} zile</div><div class="v num">{fmt(inRange)}</div></div>
  <div class="tile card"><div class="label">Medie pe zi jucată</div><div class="v num">{avg.toFixed(1)}</div></div>
  <div class="tile card"><div class="label">Cea mai bună zi</div><div class="v num">{best.value}<small>{best.value ? best.label : ""}</small></div></div>
  <div class="tile card"><div class="label">Zile la rând</div><div class="v num">{streak}<small>{streak === 1 ? "zi" : "zile"}</small></div></div>
</section>

<section class="card chartcard">
  <div class="head"><h3>Cufere pe zi</h3><span class="legend"><i class="lt"></i>azi <i class="lo"></i>alte zile</span></div>
  <div class="chartbox">
    {#key range}<BarChart data={series} />{/key}
  </div>
</section>

<section class="card sess">
  <div class="head"><h3>Sesiuni recente</h3></div>
  {#if s.sessions.length === 0}
    <div class="empty">Sesiunile apar aici după primul cufăr numărat.</div>
  {:else}
    <table>
      <thead><tr><th>Început</th><th>Durată</th><th>Cufere</th><th>Pe oră</th></tr></thead>
      <tbody>
        {#each s.sessions as x (x.id)}
          {@const ms = Math.max(60_000, x.end - x.start)}
          <tr>
            <td><Icon name="calendar" size={14} /> {new Date(x.start).toLocaleString("ro-RO", { weekday: "short", day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" })}</td>
            <td>{duration(ms)}</td>
            <td class="num c">{x.count}</td>
            <td class="num">{(x.count / (ms / 3_600_000)).toFixed(1)}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</section>

<style>
  .top { display: flex; align-items: flex-end; justify-content: space-between; margin-bottom: 20px; }
  h1 { font-size: 30px; }
  .muted { color: var(--dim); margin: 4px 0 0; }
  .seg { display: flex; padding: 4px; gap: 4px; background: var(--card); border: 2px solid var(--line); border-radius: 14px; }
  .seg button { height: 34px; padding: 0 14px; border-radius: 10px; font-family: var(--display); font-weight: 600; color: var(--muted); }
  .seg button.on { background: var(--gold); color: var(--ink); box-shadow: inset 0 -3px 0 var(--gold3); }
  .tiles { display: grid; grid-template-columns: repeat(4, 1fr); gap: 16px; margin-bottom: 18px; }
  .tile { padding: 16px 18px; }
  .v { font-size: 34px; margin-top: 4px; }
  .v small { font-size: 13px; color: var(--dim); font-family: var(--body); font-weight: 500; margin-left: 8px; }
  .chartcard { padding: 18px 20px 14px; margin-bottom: 18px; }
  .chartbox { height: 260px; }
  .head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 14px; }
  .head h3 { font-size: 19px; }
  .legend { display: flex; align-items: center; gap: 6px; font-size: 12px; color: var(--dim); }
  .legend i { width: 12px; height: 12px; border-radius: 4px; border: 2px solid var(--ink); display: inline-block; margin-left: 8px; }
  .lt { background: var(--gold); }
  .lo { background: var(--orange); }
  .sess { padding: 18px 20px; }
  table { width: 100%; border-collapse: separate; border-spacing: 0 5px; }
  th { text-align: left; font-size: 12px; color: var(--dim); font-weight: 600; padding: 0 12px 4px; }
  td { padding: 10px 12px; background: rgba(56, 36, 23, 0.55); }
  td:first-child { border-radius: 12px 0 0 12px; display: flex; align-items: center; gap: 8px; }
  td:last-child { border-radius: 0 12px 12px 0; }
  td.c { color: var(--gold); font-size: 17px; }
  .empty { color: var(--dim); padding: 24px 0; text-align: center; }
</style>
