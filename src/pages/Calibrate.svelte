<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api, type Roi } from "../lib/api";
  import Icon from "../lib/Icon.svelte";
  import Meter from "../lib/Meter.svelte";
  import RoiPicker from "../lib/RoiPicker.svelte";
  import { scale } from "svelte/transition";

  const s = $derived(app.snap!);
  const live = $derived(app.live);
  const capturing = $derived(live?.capture === "capturing");
  const listening = $derived(live?.audio === "listening");

  // ── Captura pentru alegerea zonei ──
  let thumb = $state<ImageData | null>(null);
  let loadingThumb = $state(false);
  async function grabThumb() {
    loadingThumb = true;
    try {
      const buf = await api.thumbnail();
      const dv = new DataView(buf);
      const w = dv.getUint32(0, true), h = dv.getUint32(4, true);
      thumb = new ImageData(new Uint8ClampedArray(buf, 8, w * h * 4), w, h);
    } catch (e) {
      app.toast("err", String(e));
    } finally {
      loadingThumb = false;
    }
  }
  async function setRoi(r: Roi) {
    await app.saveSettings({ roi: r });
    if (s.calib.visual_ready || s.calib.visual_samples > 0) {
      app.toast("info", "Ai schimbat zona: refă cele 3 probe vizuale.");
    }
  }

  // ── Previzualizarea zonei (se actualizează cât timp pagina e deschisă) ──
  let cur: HTMLCanvasElement;
  let tpl: HTMLCanvasElement;
  let hasTpl = $state(false);
  let hasCur = $state(false);
  $effect(() => {
    let alive = true;
    async function loop() {
      while (alive) {
        try {
          const buf = await api.visualPreview();
          draw(buf);
        } catch { /* fără previzualizare */ }
        await new Promise((r) => setTimeout(r, 350));
      }
    }
    loop();
    return () => (alive = false);
  });
  function draw(buf: ArrayBuffer) {
    const dv = new DataView(buf);
    const w = dv.getUint32(0, true), h = dv.getUint32(4, true);
    if (!w || !h) { hasCur = false; return; }
    const tplOk = dv.getUint8(8) === 1;
    const px = new Uint8Array(buf, 9);
    hasCur = true;
    hasTpl = tplOk;
    const paint = (c: HTMLCanvasElement | undefined, plane: number, mask: boolean) => {
      if (!c) return;
      c.width = w; c.height = h;
      const img = new ImageData(w, h);
      for (let i = 0; i < w * h; i++) {
        const v = px[plane * w * h + i];
        const m = mask && tplOk ? px[2 * w * h + i] : 0;
        const o = i * 4;
        if (m) {
          img.data[o] = Math.min(255, v * 0.45 + 255 * 0.55);
          img.data[o + 1] = Math.min(255, v * 0.45 + 197 * 0.55);
          img.data[o + 2] = v * 0.45 + 52 * 0.55;
        } else {
          img.data[o] = v; img.data[o + 1] = v * 0.93; img.data[o + 2] = v * 0.86;
        }
        img.data[o + 3] = 255;
      }
      c.getContext("2d")!.putImageData(img, 0, 0);
    };
    paint(cur, 0, false);
    if (tplOk) paint(tpl, 1, true);
  }

  // ── Probe cu numărătoare inversă (ca să ai timp să treci în joc) ──
  let countdown = $state<{ kind: "visual" | "audio"; n: number } | null>(null);
  function delayed(kind: "visual" | "audio") {
    countdown = { kind, n: 5 };
    const t = setInterval(() => {
      if (!countdown) return clearInterval(t);
      countdown = { kind, n: countdown.n - 1 };
      if (countdown.n <= 0) {
        clearInterval(t);
        countdown = null;
        sample(kind);
      }
    }, 1000);
  }
  async function sample(kind: "visual" | "audio") {
    try {
      await (kind === "visual" ? api.visualSample() : api.audioSample());
    } catch (e) {
      app.toast("err", String(e));
    }
  }
  async function clear(kind: "visual" | "audio") {
    await (kind === "visual" ? api.visualClear() : api.audioClear());
    await app.reload();
    app.toast("info", kind === "visual" ? "Probele vizuale au fost șterse." : "Sunetul învățat a fost șters.");
  }

  const vDone = $derived(Math.min(3, s.calib.visual_samples));
  const aDone = $derived(s.calib.audio_ready ? 2 : Math.min(2, s.calib.audio_samples));
</script>

<div class="top">
  <h1>Calibrare</h1>
  <p class="muted">Aplicația învață cum arată promptul cufărului și cum sună deschiderea lui, chiar pe jocul tău (rezoluția, limba și setările tale). Durează cam un minut.</p>
</div>

<div class="grid">
  <!-- Pasul 1 -->
  <section class="card step">
    <div class="sh"><span class="n">1</span><h3>Zona promptului</h3></div>
    <p>Când te apropii de un cufăr, jocul arată promptul de căutare (tasta + textul). Încadrează-l în dreptunghiul auriu, cu puțin spațiu în jur.</p>
    <RoiPicker roi={s.settings.roi} image={thumb} onchange={setRoi} />
    <div class="btns">
      <button class="btn small" disabled={!capturing || loadingThumb} onclick={grabThumb}><Icon name="camera" size={16} /> {loadingThumb ? "Iau captura…" : "Captură din joc"}</button>
      <button class="btn small dark" onclick={() => setRoi({ x0: 0.38, y0: 0.5, x1: 0.7, y1: 0.72 })}>Zona implicită</button>
    </div>
    {#if !capturing}<p class="warn"><Icon name="info" size={15} /> {live?.capture === "unsupported" ? "Captura merge doar pe Windows." : "Pornește Fortnite ca să pot face captura."}</p>{/if}
  </section>

  <!-- Pasul 2 -->
  <section class="card step">
    <div class="sh"><span class="n">2</span><h3>Promptul cufărului</h3>
      {#if s.calib.visual_ready}<span class="ok" in:scale><Icon name="check" size={14} /> Gata</span>{/if}
    </div>
    <p>Stai lângă un cufăr <b>închis</b> până apare promptul, apoi apasă <span class="k">{s.settings.hotkey_visual_sample}</span>. Fă asta la <b>3 cufere diferite</b>, în locuri diferite, ca să separ promptul de decor.</p>
    <div class="dots">
      {#each [0, 1, 2] as i}<span class="d" class:on={i < vDone}>{#if i < vDone}<Icon name="check" size={14} />{:else}{i + 1}{/if}</span>{/each}
      <span class="dl">{s.calib.visual_samples} {s.calib.visual_samples === 1 ? "probă" : "probe"}{s.calib.visual_ready ? ` · potrivire ${(s.calib.visual_score * 100).toFixed(0)}%` : ""}</span>
    </div>
    <div class="prev">
      <figure><canvas bind:this={cur}></canvas>{#if !hasCur}<span class="pe">Nicio imagine încă</span>{/if}<figcaption>Ce văd acum</figcaption></figure>
      <figure><canvas bind:this={tpl}></canvas>{#if !hasTpl}<span class="pe">Apare după 3 probe</span>{/if}<figcaption>Ce am învățat (auriu)</figcaption></figure>
    </div>
    <Meter value={live?.visual_score ?? 0} threshold={s.calib.visual_ready ? s.calib.visual_threshold : null} active={!!live?.prompt_visible} />
    <div class="btns">
      <button class="btn small" disabled={!capturing} onclick={() => sample("visual")}>Probă acum</button>
      <button class="btn small orange" disabled={!capturing || !!countdown} onclick={() => delayed("visual")}>
        {countdown?.kind === "visual" ? `Probă în ${countdown.n}…` : "Probă în 5 s"}
      </button>
      <button class="btn small dark" onclick={() => clear("visual")}><Icon name="trash" size={15} /></button>
    </div>
  </section>

  <!-- Pasul 3 -->
  <section class="card step">
    <div class="sh"><span class="n">3</span><h3>Sunetul cufărului</h3>
      {#if s.calib.audio_ready}<span class="ok" in:scale><Icon name="check" size={14} /> Gata</span>{/if}
    </div>
    <p>Deschide un cufăr și apasă <span class="k">{s.settings.hotkey_audio_sample}</span> <b>imediat după</b> (în 2–3 secunde). Repetă la încă un cufăr. Ține volumul jocului la un nivel normal.</p>
    <div class="dots">
      {#each [0, 1] as i}<span class="d" class:on={i < aDone}>{#if i < aDone}<Icon name="check" size={14} />{:else}{i + 1}{/if}</span>{/each}
      <span class="dl">{s.calib.audio_ready ? `potrivire ${(s.calib.audio_score * 100).toFixed(0)}%` : listening ? `ascult: ${live?.audio_device}` : "nu ascult încă"}</span>
    </div>
    <Meter value={live?.audio_score ?? 0} color="var(--orange)" threshold={s.calib.audio_ready ? (s.settings.audio_threshold ?? Math.min(0.85, Math.max(0.45, s.calib.audio_score * 0.8))) : null} />
    <div class="btns">
      <button class="btn small" disabled={!listening} onclick={() => sample("audio")}>Probă acum</button>
      <button class="btn small orange" disabled={!listening || !!countdown} onclick={() => delayed("audio")}>
        {countdown?.kind === "audio" ? `Probă în ${countdown.n}…` : "Probă în 5 s"}
      </button>
      <button class="btn small dark" onclick={() => clear("audio")}><Icon name="trash" size={15} /></button>
    </div>
  </section>

  <!-- Pasul 4 -->
  <section class="card step test">
    <div class="sh"><span class="n">4</span><h3>Testează</h3></div>
    <p>Deschide câteva cufere. Fiecare trebuie să apară o singură dată. Dacă numără greșit, corectezi cu <span class="k">{s.settings.hotkey_add}</span> / <span class="k">{s.settings.hotkey_sub}</span> și ajustezi sensibilitatea în Setări.</p>
    <div class="testbox">
      <div><div class="label">Sesiune</div><div class="tv num">{s.session}</div></div>
      <div class="sig">
        <span class="sg" class:lit={live?.prompt_visible}><Icon name="eye" size={16} /> Prompt</span>
        <span class="sg" class:lit={live?.key_down}><Icon name="key" size={16} /> Tasta {s.settings.interact_label}</span>
        <span class="sg" class:lit={(live?.audio_score ?? 0) > 0.5}><Icon name="ear" size={16} /> Sunet</span>
      </div>
    </div>
    <div class="tips">
      <div><Icon name="window" size={16} /> Pentru overlay, pune jocul pe <b>Windowed Fullscreen</b>.</div>
      <div><Icon name="shield" size={16} /> Aplicația doar privește ecranul și ascultă sunetul, ca OBS. Nu atinge jocul.</div>
    </div>
  </section>
</div>

<style>
  .top { margin-bottom: 20px; max-width: 760px; }
  h1 { font-size: 30px; }
  .muted { color: var(--dim); margin: 6px 0 0; line-height: 1.5; }
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 18px; }
  .step { padding: 18px 20px; display: flex; flex-direction: column; gap: 12px; }
  .step p { margin: 0; color: var(--muted); line-height: 1.5; font-size: 13.5px; }
  .step p b { color: var(--text); }
  .sh { display: flex; align-items: center; gap: 10px; }
  .sh h3 { font-size: 19px; }
  .n { width: 30px; height: 30px; border-radius: 10px; display: grid; place-items: center; background: var(--gold); color: var(--ink); border: 2.5px solid var(--ink); font-family: var(--display); font-weight: 700; box-shadow: 0 3px 0 var(--ink); }
  .ok { margin-left: auto; display: inline-flex; align-items: center; gap: 4px; padding: 3px 10px; border-radius: 99px; background: var(--green); color: var(--ink); border: 2px solid var(--ink); font-weight: 700; font-size: 12px; }
  .k { display: inline-block; padding: 0 7px; border-radius: 6px; background: var(--card3); border: 2px solid var(--ink); font-family: var(--display); font-weight: 700; color: var(--gold); box-shadow: 0 2px 0 var(--ink); }
  .btns { display: flex; gap: 10px; flex-wrap: wrap; }
  .warn { display: flex; align-items: center; gap: 6px; color: var(--gold) !important; font-size: 12.5px !important; }
  .dots { display: flex; align-items: center; gap: 8px; }
  .d { width: 30px; height: 30px; border-radius: 50%; display: grid; place-items: center; background: var(--card3); border: 2.5px solid var(--ink); color: var(--dim); font-weight: 700; font-family: var(--display); transition: background 0.3s, transform 0.3s cubic-bezier(0.3, 1.8, 0.5, 1); }
  .d.on { background: var(--green); color: var(--ink); transform: scale(1.08); }
  .dl { font-size: 12.5px; color: var(--dim); margin-left: 4px; }
  .prev { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
  figure { position: relative; margin: 0; }
  canvas { width: 100%; aspect-ratio: 192 / 85; display: block; border-radius: 10px; border: 2.5px solid var(--ink); background: #0d0806; }
  .pe { position: absolute; inset: 0 0 22px 0; display: grid; place-items: center; font-size: 12px; color: var(--dim); }
  figcaption { font-size: 11.5px; color: var(--dim); margin-top: 5px; text-align: center; }
  .testbox { display: flex; align-items: center; gap: 22px; padding: 14px 16px; border-radius: 14px; background: rgba(56, 36, 23, 0.55); }
  .tv { font-size: 46px; color: var(--gold); line-height: 1; }
  .sig { display: flex; flex-direction: column; gap: 6px; }
  .sg { display: inline-flex; align-items: center; gap: 7px; padding: 4px 10px; border-radius: 10px; border: 2px solid var(--line2); color: var(--dim); font-size: 13px; font-weight: 600; transition: all 0.12s; }
  .sg.lit { background: var(--gold); border-color: var(--ink); color: var(--ink); }
  .tips { display: flex; flex-direction: column; gap: 8px; font-size: 12.5px; color: var(--muted); }
  .tips div { display: flex; gap: 8px; align-items: center; }
</style>
