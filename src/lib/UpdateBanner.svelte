<script lang="ts">
  import { updater } from "./updater.svelte";
  import Icon from "./Icon.svelte";
  import { fly } from "svelte/transition";

  const show = $derived(
    !updater.dismissed && (updater.status === "available" || updater.status === "downloading" || updater.status === "installing"),
  );
  const busy = $derived(updater.status === "downloading" || updater.status === "installing");
</script>

{#if show}
  <div class="upd" transition:fly={{ y: -16, duration: 280 }}>
    <span class="ic"><Icon name="bolt" size={20} /></span>
    <div class="txt">
      <b>Versiune nouă: v{updater.version}</b>
      {#if busy}
        <div class="bar"><div class="fill" style="transform:scaleX({updater.progress})"></div></div>
        <span class="small">{updater.status === "installing" ? "Instalez și repornesc aplicația…" : `Descarc… ${Math.round(updater.progress * 100)}%`}</span>
      {:else}
        <span class="small">{updater.waitingForGame ? "Se instalează singură când închizi Fortnite. Sau acum, cu un clic." : updater.notes ? updater.notes.split("\n")[0] : "Datele și calibrarea tale rămân neschimbate."}</span>
      {/if}
    </div>
    {#if !busy}
      <button class="btn small green" onclick={() => updater.install()}>Actualizează acum</button>
      <button class="x" aria-label="Mai târziu" onclick={() => (updater.dismissed = true)}><Icon name="x" size={16} /></button>
    {/if}
  </div>
{/if}

<style>
  .upd {
    display: flex; align-items: center; gap: 14px;
    margin: 16px 28px 0; padding: 12px 14px;
    border-radius: 16px; border: 2.5px solid var(--ink);
    background: linear-gradient(90deg, #4a3017, #3a2414);
    box-shadow: 0 5px 0 var(--ink), inset 0 0 0 2px rgba(255, 197, 52, 0.35);
  }
  .ic { width: 40px; height: 40px; border-radius: 12px; display: grid; place-items: center; background: var(--gold); color: var(--ink); border: 2.5px solid var(--ink); flex: none; animation: bob 1.6s ease-in-out infinite; }
  @keyframes bob { 50% { transform: translateY(-3px) rotate(-6deg); } }
  .txt { flex: 1; display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  .txt b { font-family: var(--display); font-size: 16px; color: var(--gold2); }
  .small { font-size: 12.5px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .bar { height: 12px; border-radius: 8px; background: #120a07; border: 2px solid var(--ink); overflow: hidden; }
  .fill { height: 100%; background: linear-gradient(180deg, #b5f27e, var(--green)); transform-origin: left; transition: transform 0.2s; }
  .x { width: 32px; height: 32px; border-radius: 10px; display: grid; place-items: center; color: var(--muted); }
  .x:hover { background: var(--card3); color: var(--text); }
</style>
