<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api, type AutoDiag, type Roi } from "../lib/api";
  import Icon from "../lib/Icon.svelte";
  import Meter from "../lib/Meter.svelte";
  import RoiPicker from "../lib/RoiPicker.svelte";
  import { scale } from "svelte/transition";

  const s = $derived(app.snap!);
  const live = $derived(app.live);
  const capturing = $derived(live?.capture === "capturing");
  const listening = $derived(live?.audio === "listening");
  const auto = $derived(s.settings.visual_method === "auto");
  const ocrBad = $derived(live?.ocr === "missing" || live?.ocr === "error");
  // ── Diagnosticul detectorului automat: ce vede aplicația, live ──
  let diagCanvas = $state<HTMLCanvasElement>();
  let diag = $state<AutoDiag | null>(null);
  let hasDiagImg = $state(false);
  $effect(() => {
    if (!auto) return;
    let alive = true;
    (async () => {
      while (alive) {
        try {
          const [d, img] = await Promise.all([api.autoDiag(), api.autoDiagImage()]);
          diag = d;
          drawDiag(img, d);
        } catch { /* fără diagnostic încă */ }
        await new Promise((r) => setTimeout(r, 450));
      }
    })();
    return () => (alive = false);
  });
  function drawDiag(buf: ArrayBuffer, d: AutoDiag) {
    const c = diagCanvas;
    const dv = new DataView(buf);
    const w = dv.getUint32(0, true), h = dv.getUint32(4, true);
    hasDiagImg = w > 0 && h > 0;
    if (!c || !hasDiagImg) return;
    c.width = w; c.height = h;
    const g = c.getContext("2d")!;
    g.putImageData(new ImageData(new Uint8ClampedArray(buf, 8, w * h * 4), w, h), 0, 0);
    const k = w / Math.max(1, d.diag.region_w);
    g.lineWidth = 2;
    g.font = "bold 12px Fredoka, sans-serif";
    for (const cd of d.diag.cands) {
      const color = cd.matched ? (cd.fallback ? "#5bc0ff" : "#7ed957") : cd.ocr ? "#ffc534" : "rgba(255,255,255,0.55)";
      const pad = 3;
      g.strokeStyle = color;
      g.strokeRect(cd.x * k - pad, cd.y * k - pad, cd.w * k + pad * 2, cd.h * k + pad * 2);
      if (cd.ocr) {
        const label = (cd.text || "(nimic)").slice(0, 28) + (cd.matched ? ` · auriu ${Math.round(cd.gold * 100)}%` : "");
        const tw = g.measureText(label).width;
        const lx = Math.min(cd.x * k - pad, w - tw - 8), ly = Math.max(14, cd.y * k - pad - 4);
        g.fillStyle = "rgba(20,12,8,0.85)";
        g.fillRect(lx, ly - 12, tw + 8, 15);
        g.fillStyle = color;
        g.fillText(label, lx + 4, ly);
      }
    }
  }
  let savingDiag = $state(false);
  async function saveDiag() {
    savingDiag = true;
    try {
      const dir = await api.saveDiagnostic();
      app.toast("ok", `Diagnostic salvat în ${dir}. Trimite-mi imaginea și fișierul .json.`);
    } catch (e) {
      app.toast("err", String(e));
    } finally {
      savingDiag = false;
    }
  }
  const keyAgo = $derived(diag?.key_ms_ago);

  async function setMethod(m: "auto" | "template") {
    await app.saveSettings({ visual_method: m });
    await app.reload();
  }

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
  let cur = $state<HTMLCanvasElement>();
  let tpl = $state<HTMLCanvasElement>();
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
  <p class="muted">{auto
    ? "Promptul cufărului îl recunosc singur. Opțional, învăț și sunetul deschiderii, ca numărătoarea să fie și mai sigură."
    : "Aplicația învață cum arată promptul cufărului și cum sună deschiderea lui, chiar pe jocul tău (rezoluția, limba și setările tale). Durează cam un minut."}</p>
  <div class="seg method">
    <button class:on={auto} onclick={() => setMethod("auto")}><Icon name="eye" size={16} /> Automat (recomandat)</button>
    <button class:on={!auto} onclick={() => setMethod("template")}><Icon name="target" size={16} /> Manual, cu probe</button>
  </div>
</div>

<div class="grid">
  {#if auto}
  <!-- Detectorul automat -->
  <section class="card step wide">
    <div class="sh"><span class="n">1</span><h3>Promptul cufărului</h3>
      {#if live?.ocr === "ready"}<span class="ok" in:scale><Icon name="check" size={14} /> Automat</span>{/if}
    </div>
    <p>Nu trebuie probe. Caut promptul <b>E · SEARCH</b> oriunde apare lângă cufăr și îl citesc cu recunoașterea de text din Windows. Număr doar dacă lângă prompt e un <b>cufăr auriu</b>, nu o ladă de muniție.</p>
    {#if ocrBad}<p class="warn"><Icon name="info" size={15} /> {live?.ocr_error}</p>{/if}
    {#if !capturing}<p class="warn"><Icon name="info" size={15} /> {live?.capture === "unsupported" ? "Captura merge doar pe Windows." : "Pornește Fortnite ca să văd promptul."}</p>{/if}
    <div class="diagwrap">
      <figure class="diagfig">
        <canvas class="diag" bind:this={diagCanvas}></canvas>
        {#if !hasDiagImg}<span class="pe">{capturing ? "Aștept primul cadru…" : "Nu văd jocul încă"}</span>{/if}
        <figcaption>Ce vede aplicația (zona din centrul ecranului). <b class="lg g">verde</b> = SEARCH găsit · <b class="lg b">albastru</b> = găsit la apăsarea tastei · <b class="lg y">galben</b> = citit, alt text · <b class="lg w">alb</b> = necitit</figcaption>
      </figure>
      <div class="diagside">
        <div class="kv"><span>Captură</span><b class:bad={!capturing}>{capturing ? `${live?.capture_target || "Fortnite"} · ${live?.frame_w}×${live?.frame_h} · ${live?.fps.toFixed(0)}/s` : live?.capture_error || "nu văd jocul"}</b></div>
        <div class="kv"><span>OCR Windows</span><b class:bad={ocrBad || live?.ocr === "off"}>{live?.ocr === "ready" ? `pornit (${diag?.diag.ocr_lang || "?"})` : live?.ocr === "off" ? "nepornit" : "lipsește"}</b></div>
        <div class="kv"><span>Tasta {s.settings.interact_label}</span><b>{live?.key_down ? "apăsată acum" : keyAgo != null && keyAgo < 60000 ? `acum ${(keyAgo / 1000).toFixed(1)} s` : "neapăsată"}</b></div>
        <div class="kv"><span>Candidați / citiri</span><b>{diag ? `${diag.diag.cands.length} · ${diag.diag.ocr_runs} citiri · ${diag.diag.find_ms.toFixed(0)}+${diag.diag.ocr_ms.toFixed(0)} ms` : "—"}</b></div>
        <div class="kv col"><span>La apăsarea tastei am citit</span><b class="mono">{diag?.diag.fallback_text || "—"}</b></div>
        <div class="btns">
          <button class="btn small orange" disabled={savingDiag} onclick={saveDiag}><Icon name="camera" size={15} /> Salvează diagnostic ({s.settings.hotkey_diag || "F5"})</button>
          <button class="btn small dark" onclick={() => api.openDiagnostics()}>Deschide folderul</button>
        </div>
        <p class="hintsm">Nu vede cufărul? Stai în fața lui cu promptul pe ecran și apasă <b>{s.settings.hotkey_diag || "F5"}</b> din joc. Trimite-mi apoi imaginea și fișierul din folder.</p>
      </div>
    </div>
    <div class="facts">
      <span class="sg" class:lit={!!live?.prompt_text}><Icon name="eye" size={16} /> {live?.prompt_text ? `Citesc: ${live.prompt_text}` : "Nu văd promptul"}</span>
      <span class="sg" class:lit={!!live?.prompt_text && (live?.gold ?? 0) >= 0.06}><Icon name="chest" size={16} /> Auriu lângă prompt: {Math.round((live?.gold ?? 0) * 100)}%</span>
    </div>
    <Meter value={live?.visual_score ?? 0} threshold={0.75} active={!!live?.prompt_visible} />
  </section>
  {:else}
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
  {/if}

  <!-- Sunetul -->
  <section class="card step">
    <div class="sh"><span class="n">{auto ? 2 : 3}</span><h3>Sunetul cufărului{auto ? " (opțional)" : ""}</h3>
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
    <div class="sh"><span class="n">{auto ? 3 : 4}</span><h3>Testează</h3></div>
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
  .seg { display: flex; padding: 4px; gap: 4px; background: #170e0a; border: 2px solid var(--line); border-radius: 13px; }
  .seg button { flex: 1; height: 34px; border-radius: 9px; font-family: var(--display); font-weight: 600; font-size: 14px; color: var(--muted); transition: background 0.15s; display: inline-flex; align-items: center; justify-content: center; gap: 6px; }
  .seg button:hover { background: var(--card2); }
  .seg button.on { background: var(--gold); color: var(--ink); box-shadow: inset 0 -3px 0 var(--gold3); }
  .method { margin-top: 14px; max-width: 460px; }
  .wide { grid-column: 1 / -1; }
  .diagwrap { display: grid; grid-template-columns: minmax(0, 1.6fr) minmax(0, 1fr); gap: 16px; align-items: start; }
  .diagfig canvas.diag { aspect-ratio: auto; width: 100%; height: auto; min-height: 160px; background: #0d0806; }
  .diagfig figcaption { text-align: left; line-height: 1.5; }
  .lg { font-weight: 700; }
  .lg.g { color: #7ed957; } .lg.b { color: #5bc0ff; } .lg.y { color: #ffc534; } .lg.w { color: #e9dccd; }
  .diagside { display: flex; flex-direction: column; gap: 8px; }
  .kv { display: flex; justify-content: space-between; gap: 10px; font-size: 13px; padding: 6px 10px; border-radius: 10px; background: rgba(56, 36, 23, 0.55); }
  .kv span { color: var(--dim); }
  .kv b { color: var(--text); text-align: right; }
  .kv b.bad { color: #ff7a5c; }
  .kv.col { flex-direction: column; gap: 3px; }
  .kv.col b { text-align: left; }
  .mono { font-family: ui-monospace, monospace; font-size: 12px; word-break: break-word; }
  .hintsm { font-size: 12px !important; color: var(--dim) !important; }
  .facts { display: flex; flex-wrap: wrap; gap: 8px; }
  .testbox { display: flex; align-items: center; gap: 22px; padding: 14px 16px; border-radius: 14px; background: rgba(56, 36, 23, 0.55); }
  .tv { font-size: 46px; color: var(--gold); line-height: 1; }
  .sig { display: flex; flex-direction: column; gap: 6px; }
  .sg { display: inline-flex; align-items: center; gap: 7px; padding: 4px 10px; border-radius: 10px; border: 2px solid var(--line2); color: var(--dim); font-size: 13px; font-weight: 600; transition: all 0.12s; }
  .sg.lit { background: var(--gold); border-color: var(--ink); color: var(--ink); }
  .tips { display: flex; flex-direction: column; gap: 8px; font-size: 12.5px; color: var(--muted); }
  .tips div { display: flex; gap: 8px; align-items: center; }
</style>
