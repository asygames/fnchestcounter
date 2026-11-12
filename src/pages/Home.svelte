<script lang="ts">
  import { app, ago, duration, fmt, sourceLabel } from "../lib/state.svelte";
  import { api } from "../lib/api";
  import Chest from "../lib/Chest.svelte";
  import Burst from "../lib/Burst.svelte";
  import RollingNumber from "../lib/RollingNumber.svelte";
  import Meter from "../lib/Meter.svelte";
  import Icon from "../lib/Icon.svelte";
  import { fly, scale } from "svelte/transition";

  const s = $derived(app.snap!);
  const live = $derived(app.live);
  let now = $state(Date.now());
  $effect(() => {
    const t = setInterval(() => (now = Date.now()), 15_000);
    return () => clearInterval(t);
  });

  const perHour = $derived.by(() => {
    const h = (now - s.session_start) / 3_600_000;
    return h > 0.08 ? s.session / h : 0;
  });

  // „+1” care zboară deasupra numărului la fiecare cufăr.
  let floats = $state<{ id: number; d: number }[]>([]);
  let lastPulse = app.pulse;
  $effect(() => {
    if (app.pulse !== lastPulse) {
      lastPulse = app.pulse;
      const id = app.pulse;
      floats = [...floats, { id, d: app.lastDelta }];
      setTimeout(() => (floats = floats.filter((f) => f.id !== id)), 1100);
    }
  });

  const needsCalib = $derived(!s.calib.visual_ready && !s.calib.audio_ready);

  async function manual(delta: number) {
    const ok = await api.addManual(delta);
    if (!ok && delta < 0) app.toast("info", "Sesiunea e deja la zero.");
  }
  async function newSession() {
    await api.newSession();
    app.toast("ok", "Sesiune nouă pornită. Totalul rămâne salvat.");
  }
  const keyLabel = $derived(s.settings.interact_label || "E");
</script>

{#if needsCalib}
  <button class="calib-banner" onclick={() => (app.page = "calibrate")} in:fly={{ y: -10 }}>
    <span class="bi"><Icon name="target" size={22} /></span>
    <span><b>Calibrează detecția (durează un minut).</b> Arată-mi o dată promptul cufărului și sunetul lui, ca să le recunosc pe ecranul tău.</span>
    <span class="btn small">Începe</span>
  </button>
{/if}

<section class="hero card">
  <div class="hero-chest">
    <Burst pulse={app.pulse} />
    <Chest pulse={app.pulse} size={200} />
  </div>
  <div class="hero-main">
    <div class="label">Cufere în sesiunea asta</div>
    <div class="big">
      <RollingNumber value={s.session} size={112} gold outline={3} />
      {#each floats as f (f.id)}
        <span class="float num" class:neg={f.d < 0} in:scale={{ start: 0.4, duration: 250 }}>{f.d > 0 ? "+1" : "−1"}</span>
      {/each}
    </div>
    <div class="sub">
      <span class="pill"><Icon name="clock" size={14} /> {duration(now - s.session_start)}</span>
      <span class="pill"><Icon name="bolt" size={14} /> {perHour.toFixed(1)} / oră</span>
    </div>
    <div class="actions">
      <button class="btn green" onclick={() => manual(1)} title="Adaugă un cufăr ({s.settings.hotkey_add})"><Icon name="plus" size={18} /> Adaugă</button>
      <button class="btn dark icon" onclick={() => manual(-1)} title="Scade un cufăr ({s.settings.hotkey_sub})"><Icon name="minus" size={18} /></button>
      <button class="btn orange" onclick={newSession}><Icon name="refresh" size={18} /> Sesiune nouă</button>
    </div>
  </div>
</section>

<section class="tiles">
  <div class="tile card">
    <span class="ti" style="--c:var(--gold)"><Icon name="calendar" size={22} /></span>
    <div><div class="label">Azi</div><RollingNumber value={s.today} size={38} /></div>
  </div>
  <div class="tile card">
    <span class="ti" style="--c:var(--orange)"><Icon name="chest" size={22} /></span>
    <div><div class="label">Total</div><RollingNumber value={s.total} size={38} /></div>
  </div>
  <div class="tile card">
    <span class="ti" style="--c:var(--pink)"><Icon name="trophy" size={22} /></span>
    <div>
      <div class="label">Record pe o zi</div>
      <div class="rec"><RollingNumber value={s.best_day?.[1] ?? 0} size={38} />
        {#if s.best_day}<span class="recd">{new Date(s.best_day[0]).toLocaleDateString("ro-RO", { day: "numeric", month: "short" })}</span>{/if}
      </div>
    </div>
  </div>
</section>

<section class="row">
  <div class="card livecard">
    <div class="head"><h3>Detecție live</h3>
      {#if live && live.capture === "capturing"}<span class="pill"><span class="dot on"></span>{live.fps.toFixed(0)} cadre/s · {live.analyze_ms.toFixed(1)} ms</span>{/if}
    </div>
    <div class="det">
      <div class="dl"><Icon name="eye" size={18} /> Imagine
        {#if live?.prompt_visible}<span class="tag gold" in:scale>văd un cufăr</span>{/if}
      </div>
      <Meter value={live?.visual_score ?? 0} threshold={s.calib.visual_ready ? s.calib.visual_threshold : null} active={!!live?.prompt_visible} />
      <div class="hint">
        {#if !s.settings.visual_enabled}Oprită din setări
        {:else if live?.capture !== "capturing"}{live?.capture === "unsupported" ? "Merge doar pe Windows" : live?.capture_error || "Aștept fereastra Fortnite"}
        {:else if s.calib.visual_auto && (live?.ocr === "missing" || live?.ocr === "error")}{live.ocr_error}
        {:else if s.calib.visual_auto}{live?.prompt_text ? `Citesc „${live.prompt_text}” · auriu ${Math.round(live.gold * 100)}%` : "Caut promptul SEARCH (automat)"}
        {:else if !s.calib.visual_ready}Necalibrată: fă 3 probe în Calibrare
        {:else}Potrivire cu promptul cufărului{/if}
      </div>
    </div>
    <div class="det">
      <div class="dl"><Icon name="ear" size={18} /> Sunet</div>
      <Meter value={live?.audio_score ?? 0} threshold={s.calib.audio_ready ? (s.settings.audio_threshold ?? Math.min(0.85, Math.max(0.45, s.calib.audio_score * 0.8))) : null} color="var(--orange)" />
      <div class="hint">
        {#if !s.settings.audio_enabled}Oprit din setări
        {:else if live?.audio === "listening"}{s.calib.audio_ready ? `Ascult: ${live.audio_device}` : "Necalibrat: fă 2 probe în Calibrare"}
        {:else if live?.audio === "unsupported"}Merge doar pe Windows
        {:else}Nu pot asculta ieșirea audio{/if}
      </div>
    </div>
    <div class="keyrow">
      <span class="kbd" class:down={live?.key_down}>{keyLabel}</span>
      <span class="hint">Tasta de interacțiune {s.settings.gamepad ? "(și butonul X de pe controller)" : ""}</span>
    </div>
  </div>

  <div class="card recent">
    <div class="head"><h3>Ultimele cufere</h3><span class="pill">{s.events.length ? fmt(s.events.length) : "0"}</span></div>
    {#if s.events.length === 0}
      <div class="empty">Încă nimic. Deschide un cufăr în joc!</div>
    {:else}
      <ul>
        {#each s.events.slice(0, 4) as e (e.ts)}
          <li in:fly={{ x: -16, duration: 300 }}>
            <span class="src {e.source}"><Icon name={e.source === "audio" ? "ear" : e.source === "manual" ? "key" : "eye"} size={14} /></span>
            <span class="what">{e.delta > 0 ? "Cufăr deschis" : "Corecție −1"}</span>
            <span class="by">{sourceLabel[e.source]}</span>
            <span class="when">{ago(e.ts, now)}</span>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</section>

<style>
  .calib-banner {
    width: 100%; display: flex; align-items: center; gap: 14px; text-align: left;
    padding: 14px 16px; margin-bottom: 18px; border-radius: 16px;
    background: linear-gradient(90deg, rgba(255, 138, 31, 0.22), rgba(255, 197, 52, 0.1));
    border: 2px solid var(--orange); color: var(--text); line-height: 1.4;
  }
  .calib-banner > span:nth-child(2) { flex: 1; }
  .bi { width: 40px; height: 40px; border-radius: 12px; display: grid; place-items: center; background: var(--orange); color: #fff; border: 2px solid var(--ink); flex: none; }

  .hero { display: flex; align-items: center; gap: 30px; padding: 18px 34px; position: relative; overflow: visible; }
  .hero-chest { position: relative; width: 200px; height: 200px; flex: none; }
  .hero-main { flex: 1; min-width: 0; }
  .big { position: relative; margin: 2px 0 6px; display: flex; }
  .float {
    position: absolute; left: 100%; top: 10px; margin-left: 10px;
    font-size: 44px; color: var(--green);
    filter: drop-shadow(2px 0 0 var(--ink)) drop-shadow(-2px 0 0 var(--ink)) drop-shadow(0 2px 0 var(--ink)) drop-shadow(0 -2px 0 var(--ink)) drop-shadow(0 4px 0 var(--ink));
    animation: rise 1.1s ease-out forwards;
  }
  .float.neg { color: var(--red); }
  @keyframes rise { 0% { transform: translateY(20px); opacity: 0; } 20% { opacity: 1; } 100% { transform: translateY(-60px); opacity: 0; } }
  .sub { display: flex; gap: 8px; margin-bottom: 18px; }
  .actions { display: flex; gap: 12px; }

  .tiles { display: grid; grid-template-columns: repeat(3, 1fr); gap: 18px; margin: 16px 0; }
  .tile { display: flex; align-items: center; gap: 16px; padding: 12px 18px; }
  .ti { width: 50px; height: 50px; border-radius: 15px; display: grid; place-items: center; background: var(--c); color: var(--ink); border: 2.5px solid var(--ink); box-shadow: 0 4px 0 var(--ink), inset 0 2px 0 rgba(255, 255, 255, 0.45); flex: none; }
  .tile :global(.roll) { color: var(--text); }
  .rec { display: flex; align-items: baseline; gap: 10px; }
  .recd { font-size: 12.5px; color: var(--dim); }

  .row { display: grid; grid-template-columns: 1.1fr 1fr; gap: 18px; }
  .head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 10px; }
  .head h3 { font-size: 19px; }
  .livecard, .recent { padding: 16px 20px; }
  .det { margin-bottom: 10px; }
  .dl { display: flex; align-items: center; gap: 8px; font-weight: 600; margin-bottom: 7px; color: var(--muted); }
  .tag { margin-left: auto; font-size: 11.5px; padding: 2px 9px; border-radius: 99px; border: 2px solid var(--ink); color: var(--ink); font-weight: 700; }
  .tag.gold { background: var(--gold); }
  .hint { font-size: 12px; color: var(--dim); margin-top: 6px; }
  .keyrow { display: flex; align-items: center; gap: 12px; padding-top: 4px; }
  .keyrow .hint { margin: 0; }
  .kbd {
    min-width: 40px; height: 38px; padding: 0 10px; display: grid; place-items: center;
    font-family: var(--display); font-weight: 700; font-size: 17px;
    background: var(--card3); border: 2.5px solid var(--ink); border-radius: 10px; box-shadow: 0 4px 0 var(--ink);
    transition: transform 0.06s, background 0.1s, box-shadow 0.06s;
  }
  .kbd.down { transform: translateY(3px); box-shadow: 0 1px 0 var(--ink); background: var(--gold); color: var(--ink); }

  ul { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }
  li { display: grid; grid-template-columns: 28px 1fr auto auto; align-items: center; gap: 10px; padding: 8px 10px; border-radius: 12px; background: rgba(56, 36, 23, 0.55); }
  .src { width: 28px; height: 28px; border-radius: 9px; display: grid; place-items: center; border: 2px solid var(--ink); color: var(--ink); background: var(--gold); }
  .src.audio { background: var(--orange); }
  .src.manual { background: var(--muted); }
  .src.both { background: linear-gradient(135deg, var(--gold) 50%, var(--orange) 50%); }
  .what { font-weight: 600; }
  .by { font-size: 12px; color: var(--dim); }
  .when { font-size: 12px; color: var(--muted); min-width: 72px; text-align: right; }
  .empty { color: var(--dim); padding: 30px 0; text-align: center; }
</style>
