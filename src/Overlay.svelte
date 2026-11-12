<script lang="ts">
  // Overlay-ul mic de peste joc: cufăr + numărul din sesiune + azi.
  import { onMount } from "svelte";
  import { api, isTauri, listen, type ChestPayload, type Settings } from "./lib/api";
  import Chest from "./lib/Chest.svelte";
  import RollingNumber from "./lib/RollingNumber.svelte";

  let session = $state(0);
  let today = $state(0);
  let pulse = $state(0);
  let bump = $state(false);
  let editing = $state(false);
  let zoom = $state(1);
  let reduce = $state(false);

  async function load() {
    const s = await api.snapshot();
    session = s.session;
    today = s.today;
    zoom = s.settings.overlay_scale;
    reduce = s.settings.reduce_motion;
  }

  onMount(() => {
    load();
    // Doar în modul demo din browser: overlay.html#edit arată modul de mutare.
    if (!isTauri && location.hash === "#edit") editing = true;
    const subs = [
      listen<ChestPayload>("chest", (p) => {
        session = p.session;
        today = p.today;
        if (p.delta > 0) {
          pulse++;
          bump = false;
          requestAnimationFrame(() => (bump = true));
          setTimeout(() => (bump = false), 700);
        }
      }),
      listen<null>("refresh", load),
      listen<Settings>("settings", (s) => {
        zoom = s.overlay_scale;
        reduce = s.reduce_motion;
      }),
      listen<boolean>("overlay-edit", (e) => (editing = e)),
    ];
    return () => subs.forEach((p) => p.then((u) => u()));
  });
</script>

<div class="wrap" class:reduce-motion={reduce} style="zoom:{zoom}" data-tauri-drag-region>
  <div class="ov" class:editing class:bump data-tauri-drag-region>
    <div class="chest" data-tauri-drag-region><Chest {pulse} size={66} /></div>
    <div class="txt" data-tauri-drag-region>
      <div class="n"><RollingNumber value={session} size={40} gold outline={2} /></div>
      <div class="lbl" data-tauri-drag-region>sesiune · <b>{today}</b> azi</div>
    </div>
    {#if editing}
      <div class="drag" data-tauri-drag-region>trage-mă</div>
      <button class="done" onclick={() => api.overlayEdit(false)}>Gata</button>
    {/if}
  </div>
</div>

<style>
  :global(html), :global(body) { background: transparent !important; }
  .wrap { width: 250px; height: 96px; padding: 8px; }
  .ov {
    position: relative;
    height: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 16px 0 8px;
    border-radius: 20px;
    background: linear-gradient(180deg, rgba(48, 30, 19, 0.92), rgba(28, 18, 12, 0.92));
    border: 2.5px solid #2a1208;
    box-shadow: 0 4px 0 #2a1208, inset 0 0 0 2px rgba(255, 197, 52, 0.35);
    transition: transform 0.2s;
  }
  .ov.bump { animation: bump 0.6s cubic-bezier(0.3, 1.8, 0.5, 1); }
  @keyframes bump { 30% { transform: scale(1.08); } 100% { transform: scale(1); } }
  .ov.editing { outline: 3px dashed var(--gold); outline-offset: 2px; cursor: move; }
  .chest { margin-top: -6px; }
  .txt { display: flex; flex-direction: column; line-height: 1; }
  .lbl { font-size: 12px; font-weight: 600; color: #e9cfb3; margin-top: 2px; letter-spacing: 0.3px; }
  .lbl b { color: #fff4e4; }
  .done { position: absolute; right: 10px; top: 50%; transform: translateY(-50%); background: #7ed957; color: #2a1208; border: 2.5px solid #2a1208; border-radius: 10px; font-family: var(--display); font-size: 13px; font-weight: 700; padding: 3px 10px; box-shadow: 0 3px 0 #2a1208; cursor: pointer; }
  .done:active { transform: translateY(calc(-50% + 2px)); box-shadow: 0 1px 0 #2a1208; }
  .drag { position: absolute; left: 14px; top: -4px; transform: translateY(-50%); background: var(--gold); color: #2a1208; border: 2px solid #2a1208; border-radius: 8px; font-size: 10.5px; font-weight: 800; padding: 1px 7px; }
</style>
