<script lang="ts">
  import { onMount } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { app, type Page } from "./lib/state.svelte";
  import { api, isTauri, windowAction } from "./lib/api";
  import Icon from "./lib/Icon.svelte";
  import Toasts from "./lib/Toasts.svelte";
  import Home from "./pages/Home.svelte";
  import Stats from "./pages/Stats.svelte";
  import Calibrate from "./pages/Calibrate.svelte";
  import SettingsPage from "./pages/Settings.svelte";

  let error = $state("");
  onMount(() => {
    app.init().catch((e) => (error = String(e)));
    // Doar în modul demo din browser: #stats, #calibrate, #settings, #pulse.
    if (!isTauri && location.hash) {
      const h = location.hash.slice(1);
      if (["home", "stats", "calibrate", "settings"].includes(h)) app.page = h as Page;
      if (h === "pulse") setTimeout(() => api.simulate("visual"), 700);
    }
  });

  const nav: { id: Page; label: string; icon: string }[] = [
    { id: "home", label: "Acasă", icon: "home" },
    { id: "stats", label: "Statistici", icon: "stats" },
    { id: "calibrate", label: "Calibrare", icon: "target" },
    { id: "settings", label: "Setări", icon: "settings" },
  ];

  const status = $derived.by(() => {
    const l = app.live;
    if (!l) return { cls: "", text: "Pornesc…" };
    if (l.capture === "unsupported") return { cls: "warn", text: "Detecția merge doar pe Windows" };
    if (l.capture === "error") return { cls: "err", text: "Eroare la captură" };
    if (l.capture === "disabled" && l.audio !== "listening") return { cls: "warn", text: "Detecția e oprită" };
    if (l.capture === "capturing") return { cls: "on", text: `Văd jocul · ${l.capture_target}` };
    if (l.audio === "listening") return { cls: "warn", text: "Aștept Fortnite · ascult sunetul" };
    return { cls: "warn", text: "Aștept să pornești Fortnite" };
  });

  const needsCalib = $derived(!!app.snap && !app.snap.calib.visual_ready && !app.snap.calib.audio_ready);
</script>

<div class="shell" class:reduce-motion={app.settings?.reduce_motion}>
  <header class="titlebar" data-tauri-drag-region>
    <div class="brand" data-tauri-drag-region>
      <img src="/icon.svg" alt="" width="26" height="26" />
      <span data-tauri-drag-region>FN Chest Counter</span>
    </div>
    <div class="pill status" data-tauri-drag-region><span class="dot {status.cls}"></span>{status.text}</div>
    <div class="winbtns">
      <button aria-label="Minimizează" onclick={() => windowAction("minimize")}><svg viewBox="0 0 12 12"><path d="M2 6h8" /></svg></button>
      <button aria-label="Maximizează" onclick={() => windowAction("maximize")}><svg viewBox="0 0 12 12"><rect x="2" y="2" width="8" height="8" rx="1.5" /></svg></button>
      <button class="close" aria-label="Închide" onclick={() => windowAction("close")}><svg viewBox="0 0 12 12"><path d="M2.5 2.5l7 7M9.5 2.5l-7 7" /></svg></button>
    </div>
  </header>

  <aside class="side">
    <nav>
      {#each nav as n}
        <button class="navbtn" class:active={app.page === n.id} onclick={() => (app.page = n.id)}>
          <span class="navic"><Icon name={n.icon} size={20} /></span>
          {n.label}
          {#if n.id === "calibrate" && needsCalib}<span class="badge">!</span>{/if}
        </button>
      {/each}
    </nav>
    <div class="side-foot">
      {#if app.settings}
        <button class="ovl" class:on={app.settings.overlay} onclick={() => app.saveSettings({ overlay: !app.settings!.overlay })}>
          <Icon name="layers" size={18} />
          <span>Overlay în joc</span>
          <span class="switch" class:on={app.settings.overlay}><span></span></span>
        </button>
      {/if}
      <div class="ver">v{app.snap?.version ?? ""}</div>
    </div>
  </aside>

  <main class="content">
    {#if error}
      <div class="card fatal">Nu am putut porni interfața: {error}</div>
    {:else if app.snap}
      {#key app.page}
        <div class="page" in:fly={{ y: 14, duration: 260, delay: 60 }} out:fade={{ duration: 60 }}>
          {#if app.page === "home"}<Home />
          {:else if app.page === "stats"}<Stats />
          {:else if app.page === "calibrate"}<Calibrate />
          {:else}<SettingsPage />{/if}
        </div>
      {/key}
    {/if}
  </main>
  <Toasts />
</div>

<style>
  .shell {
    display: grid;
    grid-template-columns: 228px 1fr;
    grid-template-rows: 46px 1fr;
    height: 100vh;
    background:
      radial-gradient(900px 500px at 75% -10%, rgba(255, 138, 31, 0.13), transparent 70%),
      radial-gradient(700px 500px at 0% 110%, rgba(255, 197, 52, 0.07), transparent 70%),
      var(--bg);
  }
  .titlebar {
    grid-column: 1 / -1;
    display: flex;
    align-items: center;
    gap: 16px;
    padding-left: 16px;
    border-bottom: 2px solid var(--line);
    background: rgba(20, 13, 11, 0.6);
  }
  .brand { display: flex; align-items: center; gap: 10px; font-family: var(--display); font-weight: 600; font-size: 17px; width: 196px; }
  .status { margin-right: auto; }
  .winbtns { display: flex; height: 100%; }
  .winbtns button { width: 48px; height: 100%; display: grid; place-items: center; color: var(--muted); transition: background 0.12s; }
  .winbtns button:hover { background: var(--card3); color: var(--text); }
  .winbtns .close:hover { background: var(--red); color: #fff; }
  .winbtns svg { width: 12px; height: 12px; fill: none; stroke: currentColor; stroke-width: 1.6; }

  .side { display: flex; flex-direction: column; justify-content: space-between; padding: 18px 14px; border-right: 2px solid var(--line); background: rgba(27, 18, 14, 0.55); }
  nav { display: flex; flex-direction: column; gap: 6px; }
  .navbtn {
    position: relative;
    display: flex; align-items: center; gap: 12px;
    height: 48px; padding: 0 12px; border-radius: 14px;
    font-family: var(--display); font-size: 16.5px; font-weight: 500; color: var(--muted);
    border: 2px solid transparent;
    transition: background 0.15s, color 0.15s, transform 0.15s;
  }
  .navbtn:hover { background: var(--card); color: var(--text); }
  .navbtn.active {
    color: var(--ink);
    background: linear-gradient(180deg, var(--gold2), var(--gold));
    border-color: var(--ink);
    box-shadow: 0 4px 0 var(--ink), inset 0 2px 0 rgba(255, 255, 255, 0.45);
    font-weight: 600;
  }
  .navic { display: grid; place-items: center; width: 30px; height: 30px; border-radius: 10px; background: var(--card3); color: var(--gold); border: 2px solid var(--ink); }
  .active .navic { background: var(--ink); color: var(--gold); }
  .badge { margin-left: auto; width: 22px; height: 22px; border-radius: 50%; background: var(--red); color: #fff; border: 2px solid var(--ink); display: grid; place-items: center; font-weight: 700; font-size: 13px; animation: bob 1.6s ease-in-out infinite; }
  @keyframes bob { 50% { transform: translateY(-3px); } }

  .side-foot { display: flex; flex-direction: column; gap: 12px; }
  .ovl { display: flex; align-items: center; gap: 10px; padding: 12px; border-radius: 14px; background: var(--card); border: 2px solid var(--line); color: var(--muted); font-weight: 600; font-size: 13.5px; text-align: left; }
  .ovl.on { color: var(--text); }
  .ovl span:nth-child(2) { flex: 1; }
  .switch { width: 38px; height: 22px; border-radius: 22px; background: var(--card3); border: 2px solid var(--ink); position: relative; transition: background 0.2s; flex: none; }
  .switch span { position: absolute; top: 2px; left: 2px; width: 14px; height: 14px; border-radius: 50%; background: var(--muted); transition: transform 0.25s cubic-bezier(0.3, 1.6, 0.5, 1), background 0.2s; }
  .switch.on { background: var(--green); }
  .switch.on span { transform: translateX(16px); background: #fff; }
  .ver { font-size: 11.5px; color: var(--dim); padding-left: 6px; }

  .content { position: relative; overflow: auto; }
  .page { padding: 20px 28px 26px; min-height: 100%; }
  .fatal { margin: 30px; padding: 20px; }
</style>
