<script lang="ts">
  import { app } from "../lib/state.svelte";
  import { api, type Mode, type Settings } from "../lib/api";
  import { keyToAccelerator, keyToVk, mouseToVk } from "../lib/keys";
  import Icon from "../lib/Icon.svelte";
  import { fade, scale } from "svelte/transition";

  const s = $derived(app.snap!.settings);
  const save = (p: Partial<Settings>) => app.saveSettings(p);

  const modes: { id: Mode; label: string; hint: string }[] = [
    { id: "either", label: "Oricare", hint: "Numără când vede promptul SAU aude sunetul. Cel mai sensibil." },
    { id: "both", label: "Amândouă", hint: "Numără doar când se potrivesc imaginea ȘI sunetul. Cel mai sigur." },
    { id: "visual", label: "Imagine", hint: "Folosește doar promptul de pe ecran." },
    { id: "audio", label: "Sunet", hint: "Folosește doar sunetul deschiderii." },
  ];

  // ── Legarea tastelor ──
  let binding = $state<null | "interact" | "hotkey_add" | "hotkey_sub" | "hotkey_visual_sample" | "hotkey_audio_sample">(null);
  function onKey(e: KeyboardEvent) {
    if (!binding) return;
    e.preventDefault();
    if (e.key === "Escape") { binding = null; return; }
    if (binding === "interact") {
      const k = keyToVk(e);
      if (k) { save({ interact_vk: k[0], interact_label: k[1] }); binding = null; }
      return;
    }
    if (e.key === "Backspace" || e.key === "Delete") { save({ [binding]: "" } as Partial<Settings>); binding = null; return; }
    const acc = keyToAccelerator(e);
    if (acc) { save({ [binding]: acc } as Partial<Settings>); binding = null; }
  }
  function onMouse(e: MouseEvent) {
    if (binding !== "interact") return;
    const k = mouseToVk(e.button);
    if (k && e.button !== 0) { e.preventDefault(); save({ interact_vk: k[0], interact_label: k[1] }); binding = null; }
  }

  // ── Overlay ──
  let editingOverlay = $state(false);
  async function toggleOverlayEdit() {
    editingOverlay = !editingOverlay;
    if (editingOverlay && !s.overlay) await save({ overlay: true });
    await api.overlayEdit(editingOverlay);
    if (editingOverlay) app.toast("info", "Trage overlay-ul unde vrei pe ecran, apoi apasă Gata.");
  }

  // ── Ștergerea datelor ──
  let confirmReset = $state(false);
  let resetText = $state("");
  async function doReset() {
    await api.resetAll();
    confirmReset = false;
    resetText = "";
    app.toast("ok", "Istoricul a fost șters.");
  }

  const vAuto = $derived(s.visual_threshold == null);
  const aAuto = $derived(s.audio_threshold == null);
  const vThr = $derived(s.visual_threshold ?? app.snap!.calib.visual_threshold);
  const aThr = $derived(s.audio_threshold ?? Math.min(0.85, Math.max(0.45, app.snap!.calib.audio_score * 0.8 || 0.6)));
</script>

<svelte:window onkeydown={onKey} onmousedown={onMouse} />

<div class="top">
  <h1>Setări</h1>
  <p class="muted">Se salvează automat.</p>
</div>

<div class="cols">
  <div class="col">
    <section class="card sec">
      <h3><Icon name="target" size={18} /> Detecție</h3>
      <div class="field">
        <div class="fl">Când număr un cufăr</div>
        <div class="seg">
          {#each modes as m}<button class:on={s.mode === m.id} onclick={() => save({ mode: m.id })}>{m.label}</button>{/each}
        </div>
        <div class="fh">{modes.find((m) => m.id === s.mode)?.hint}</div>
      </div>

      <label class="row"><span><Icon name="eye" size={16} /> Detecție după imagine</span><input type="checkbox" class="tg" checked={s.visual_enabled} onchange={(e) => save({ visual_enabled: e.currentTarget.checked })} /></label>
      <label class="row"><span><Icon name="ear" size={16} /> Detecție după sunet</span><input type="checkbox" class="tg" checked={s.audio_enabled} onchange={(e) => save({ audio_enabled: e.currentTarget.checked })} /></label>

      <div class="field">
        <div class="fl">Sensibilitate imagine <span class="val">{vAuto ? "automat" : `${Math.round(vThr * 100)}%`}</span></div>
        <div class="sl">
          <input type="range" min="0.3" max="0.95" step="0.01" value={vThr} style="--p:{((vThr - 0.3) / 0.65) * 100}%" disabled={vAuto}
            onchange={(e) => save({ visual_threshold: Number(e.currentTarget.value) })} />
          <label class="auto"><input type="checkbox" checked={vAuto} onchange={(e) => save({ visual_threshold: e.currentTarget.checked ? null : vThr })} /> automat</label>
        </div>
        <div class="fh">Prag mai mic = prinde mai ușor promptul, dar crește riscul de greșeli.</div>
      </div>
      <div class="field">
        <div class="fl">Sensibilitate sunet <span class="val">{aAuto ? "automat" : `${Math.round(aThr * 100)}%`}</span></div>
        <div class="sl">
          <input type="range" min="0.3" max="0.95" step="0.01" value={aThr} style="--p:{((aThr - 0.3) / 0.65) * 100}%" disabled={aAuto}
            onchange={(e) => save({ audio_threshold: Number(e.currentTarget.value) })} />
          <label class="auto"><input type="checkbox" checked={aAuto} onchange={(e) => save({ audio_threshold: e.currentTarget.checked ? null : aThr })} /> automat</label>
        </div>
      </div>

      <div class="field">
        <div class="fl">Cadre analizate pe secundă <span class="val">{s.fps}</span></div>
        <input type="range" min="5" max="30" step="1" value={s.fps} style="--p:{((s.fps - 5) / 25) * 100}%" onchange={(e) => save({ fps: Number(e.currentTarget.value) })} />
        <div class="fh">15 e suficient. Analiza unui cadru durează sub 1 ms, deci nu afectează FPS-ul jocului.</div>
      </div>

      <div class="field">
        <div class="fl">Ce capturez</div>
        <div class="seg">
          <button class:on={s.capture_source === "window"} onclick={() => save({ capture_source: "window" })}>Fereastra Fortnite</button>
          <button class:on={s.capture_source === "monitor"} onclick={() => save({ capture_source: "monitor" })}>Un monitor</button>
        </div>
        {#if s.capture_source === "monitor"}
          <div class="seg small" transition:fade={{ duration: 120 }}>
            {#each [0, 1, 2] as m}<button class:on={s.monitor_index === m} onclick={() => save({ monitor_index: m })}>Monitor {m + 1}</button>{/each}
          </div>
        {/if}
      </div>
    </section>

    <section class="card sec">
      <h3><Icon name="key" size={18} /> Tasta de interacțiune</h3>
      <p class="fh">Număr după imagine doar dacă ai apăsat tasta cu care deschizi cufere. Așa nu se numără când doar treci pe lângă un cufăr.</p>
      <div class="row">
        <span>Tasta din joc</span>
        <button class="bind" class:listening={binding === "interact"} onclick={() => (binding = binding === "interact" ? null : "interact")}>
          {binding === "interact" ? "Apasă tasta…" : s.interact_label}
        </button>
      </div>
      <label class="row"><span>Cere tasta pentru numărare</span><input type="checkbox" class="tg" checked={s.require_key} onchange={(e) => save({ require_key: e.currentTarget.checked })} /></label>
      <label class="row"><span><Icon name="gamepad" size={16} /> Controller (butonul X)</span><input type="checkbox" class="tg" checked={s.gamepad} onchange={(e) => save({ gamepad: e.currentTarget.checked })} /></label>
    </section>
  </div>

  <div class="col">
    <section class="card sec">
      <h3><Icon name="layers" size={18} /> Overlay în joc</h3>
      <label class="row"><span>Arată overlay-ul</span><input type="checkbox" class="tg" checked={s.overlay} onchange={(e) => save({ overlay: e.currentTarget.checked })} /></label>
      <div class="field">
        <div class="fl">Mărime <span class="val">{Math.round(s.overlay_scale * 100)}%</span></div>
        <input type="range" min="0.6" max="2" step="0.05" value={s.overlay_scale} style="--p:{((s.overlay_scale - 0.6) / 1.4) * 100}%" onchange={(e) => save({ overlay_scale: Number(e.currentTarget.value) })} />
      </div>
      <div class="btns">
        <button class="btn small" class:green={editingOverlay} onclick={toggleOverlayEdit}><Icon name={editingOverlay ? "check" : "move"} size={16} /> {editingOverlay ? "Gata" : "Mută overlay-ul"}</button>
        <button class="btn small dark" onclick={() => save({ overlay_x: null, overlay_y: null })}>Poziția implicită</button>
      </div>
      <p class="fh">Overlay-ul se vede peste joc doar în modul <b>Windowed Fullscreen</b> (sau fereastră). În Fullscreen exclusiv, Windows nu permite nimic deasupra jocului.</p>
    </section>

    <section class="card sec">
      <h3><Icon name="bolt" size={18} /> Scurtături globale</h3>
      <p class="fh">Merg și când ești în joc. Click pe tastă, apoi apasă combinația nouă (Backspace o șterge).</p>
      {#each [["hotkey_add", "Adaugă un cufăr (+1)"], ["hotkey_sub", "Scade un cufăr (−1)"], ["hotkey_visual_sample", "Probă vizuală (calibrare)"], ["hotkey_audio_sample", "Probă de sunet (calibrare)"]] as [k, label]}
        <div class="row">
          <span>{label}</span>
          <button class="bind" class:listening={binding === k} onclick={() => (binding = binding === k ? null : (k as typeof binding))}>
            {binding === k ? "Apasă…" : (s[k as keyof Settings] as string) || "—"}
          </button>
        </div>
      {/each}
    </section>

    <section class="card sec">
      <h3><Icon name="settings" size={18} /> Aplicație</h3>
      <label class="row"><span>La închidere rămâne în tray</span><input type="checkbox" class="tg" checked={s.close_to_tray} onchange={(e) => save({ close_to_tray: e.currentTarget.checked })} /></label>
      <label class="row"><span>Clinchet la fiecare cufăr</span><input type="checkbox" class="tg" checked={s.sounds} onchange={(e) => save({ sounds: e.currentTarget.checked })} /></label>
      <label class="row"><span>Mai puține animații</span><input type="checkbox" class="tg" checked={s.reduce_motion} onchange={(e) => save({ reduce_motion: e.currentTarget.checked })} /></label>
      <div class="data">
        <div class="fh">Datele sunt salvate local în:</div>
        <code>{app.snap!.data_dir}</code>
      </div>
      <button class="btn small red" onclick={() => (confirmReset = true)}><Icon name="trash" size={15} /> Șterge tot istoricul</button>
    </section>
  </div>
</div>

{#if confirmReset}
  <div class="modal-bg" transition:fade={{ duration: 150 }}>
    <div class="modal card" transition:scale={{ start: 0.9, duration: 220 }}>
      <h3>Ștergi tot istoricul?</h3>
      <p>Se șterg definitiv totalul ({app.snap!.total.toLocaleString("ro-RO")} cufere), zilele și sesiunile. Calibrarea și setările rămân. Nu se poate anula.</p>
      <p>Scrie <b>STERGE</b> ca să confirmi:</p>
      <input class="txt" bind:value={resetText} placeholder="STERGE" />
      <div class="btns end">
        <button class="btn small dark" onclick={() => { confirmReset = false; resetText = ""; }}>Renunță</button>
        <button class="btn small red" disabled={resetText.trim().toUpperCase() !== "STERGE"} onclick={doReset}>Șterge definitiv</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .top { display: flex; align-items: baseline; gap: 14px; margin-bottom: 20px; }
  h1 { font-size: 30px; }
  .muted { color: var(--dim); margin: 0; }
  .cols { display: grid; grid-template-columns: 1fr 1fr; gap: 18px; align-items: start; }
  .col { display: flex; flex-direction: column; gap: 18px; }
  .sec { padding: 18px 20px; display: flex; flex-direction: column; gap: 14px; }
  .sec h3 { display: flex; align-items: center; gap: 8px; font-size: 18px; color: var(--gold); }
  .field { display: flex; flex-direction: column; gap: 8px; }
  .fl { font-weight: 600; display: flex; justify-content: space-between; }
  .val { color: var(--gold); font-family: var(--display); }
  .fh { font-size: 12.5px; color: var(--dim); line-height: 1.45; margin: 0; }
  .fh b { color: var(--muted); }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 12px; font-weight: 500; cursor: pointer; }
  .row > span { display: flex; align-items: center; gap: 8px; }
  .seg { display: flex; padding: 4px; gap: 4px; background: #170e0a; border: 2px solid var(--line); border-radius: 13px; }
  .seg button { flex: 1; height: 34px; border-radius: 9px; font-family: var(--display); font-weight: 600; font-size: 14px; color: var(--muted); transition: background 0.15s; }
  .seg button:hover { background: var(--card2); }
  .seg button.on { background: var(--gold); color: var(--ink); box-shadow: inset 0 -3px 0 var(--gold3); }
  .seg.small { margin-top: 2px; }
  .sl { display: flex; align-items: center; gap: 12px; }
  .auto { display: flex; align-items: center; gap: 6px; font-size: 12.5px; color: var(--muted); white-space: nowrap; }
  input[type="range"]:disabled { opacity: 0.45; }
  .tg { appearance: none; -webkit-appearance: none; width: 44px; height: 26px; border-radius: 26px; background: var(--card3); border: 2.5px solid var(--ink); position: relative; cursor: pointer; transition: background 0.2s; flex: none; margin: 0; }
  .tg::after { content: ""; position: absolute; top: 2px; left: 2px; width: 17px; height: 17px; border-radius: 50%; background: var(--muted); transition: transform 0.25s cubic-bezier(0.3, 1.6, 0.5, 1), background 0.2s; }
  .tg:checked { background: var(--green); }
  .tg:checked::after { transform: translateX(18px); background: #fff; }
  .bind { min-width: 92px; height: 36px; padding: 0 12px; border-radius: 10px; background: var(--card3); border: 2.5px solid var(--ink); box-shadow: 0 3px 0 var(--ink); font-family: var(--display); font-weight: 700; color: var(--gold); }
  .bind.listening { background: var(--gold); color: var(--ink); animation: pulse 1s ease-in-out infinite; }
  @keyframes pulse { 50% { transform: scale(1.05); } }
  .btns { display: flex; gap: 10px; flex-wrap: wrap; }
  .btns.end { justify-content: flex-end; }
  .data code { display: block; margin-top: 4px; font-size: 11.5px; color: var(--muted); background: #170e0a; padding: 8px 10px; border-radius: 9px; word-break: break-all; user-select: text; }
  .modal-bg { position: fixed; inset: 0; background: rgba(10, 6, 4, 0.7); display: grid; place-items: center; z-index: 50; }
  .modal { width: 440px; padding: 22px 24px; display: flex; flex-direction: column; gap: 10px; }
  .modal h3 { font-size: 21px; color: var(--red); }
  .modal p { margin: 0; color: var(--muted); line-height: 1.5; }
  .txt { height: 40px; border-radius: 10px; border: 2.5px solid var(--ink); background: #170e0a; color: var(--text); padding: 0 12px; font: inherit; font-weight: 600; letter-spacing: 1px; user-select: text; }
</style>
