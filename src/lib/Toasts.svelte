<script lang="ts">
  import { app } from "./state.svelte";
  import Icon from "./Icon.svelte";
  import { fly } from "svelte/transition";
  import { flip } from "svelte/animate";
</script>

<div class="toasts">
  {#each app.toasts as t (t.id)}
    <div class="toast {t.kind}" in:fly={{ y: 24, duration: 260 }} out:fly={{ x: 40, duration: 200 }} animate:flip={{ duration: 200 }}>
      <span class="ic"><Icon name={t.kind === "err" ? "x" : t.kind === "ok" ? "check" : "info"} size={16} /></span>
      <span>{t.text}</span>
    </div>
  {/each}
</div>

<style>
  .toasts { position: fixed; right: 22px; bottom: 22px; display: flex; flex-direction: column; gap: 10px; z-index: 100; max-width: 380px; }
  .toast {
    display: flex; gap: 10px; align-items: flex-start;
    padding: 12px 14px; border-radius: 14px;
    background: var(--card3); border: 2px solid var(--ink);
    box-shadow: 0 5px 0 var(--ink);
    font-weight: 500; line-height: 1.35;
  }
  .ic { flex: none; width: 24px; height: 24px; border-radius: 50%; display: grid; place-items: center; border: 2px solid var(--ink); color: var(--ink); }
  .ok .ic { background: var(--green); }
  .err .ic { background: var(--red); color: #fff; }
  .info .ic { background: var(--gold); }
</style>
