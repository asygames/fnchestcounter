<script lang="ts">
  // Bară de scor live (0..1) cu marcajul pragului.
  let {
    value = 0,
    threshold = null,
    color = "var(--gold)",
    active = false,
  }: { value?: number; threshold?: number | null; color?: string; active?: boolean } = $props();
  const pct = $derived(Math.max(0, Math.min(1, value)) * 100);
</script>

<div class="meter" class:active>
  <div class="fill" style="transform:scaleX({pct / 100});background:{color}"></div>
  {#if threshold != null}
    <div class="thr" style="left:{Math.max(0, Math.min(1, threshold)) * 100}%"></div>
  {/if}
</div>

<style>
  .meter {
    position: relative;
    height: 14px;
    border-radius: 10px;
    background: #170e0a;
    border: 2px solid var(--ink);
    overflow: hidden;
  }
  .fill {
    position: absolute;
    inset: 0;
    transform-origin: left;
    border-radius: 8px;
    transition: transform 0.18s linear;
    box-shadow: inset 0 2px 0 rgba(255, 255, 255, 0.35);
  }
  .thr {
    position: absolute;
    top: -2px;
    bottom: -2px;
    width: 3px;
    margin-left: -1.5px;
    background: var(--text);
    box-shadow: 0 0 0 1.5px var(--ink);
  }
  .active { border-color: var(--green); }
</style>
