<script lang="ts">
  // Număr cu cifre care se rotesc (ca un contor mecanic). Fiecare cifră e o
  // coloană 0–9 mutată cu transform (rapid, fără relayout).
  // `gold` = cifre aurii cu contur închis gros (gradientul e pus pe fiecare cifră,
  // pentru că background-clip:text nu trece prin elementele animate).
  let {
    value = 0,
    size = 64,
    gold = false,
    outline = 0,
  }: { value?: number; size?: number; gold?: boolean; outline?: number } = $props();

  const digits = $derived(Math.max(0, Math.round(value)).toLocaleString("ro-RO").split(""));
  const o = $derived(outline);
  const filter = $derived(
    o > 0
      ? `drop-shadow(${o}px 0 0 #2a1208) drop-shadow(-${o}px 0 0 #2a1208) drop-shadow(0 ${o}px 0 #2a1208) drop-shadow(0 -${o}px 0 #2a1208) drop-shadow(0 ${o * 2}px 0 #2a1208)`
      : "none",
  );
</script>

<span class="wrap" style="filter:{filter}">
  <span class="roll num" class:gold style="font-size:{size}px;height:{size * 1.12}px" aria-label={String(value)}>
    {#each digits as d, i (digits.length - i)}
      {#if /[0-9]/.test(d)}
        <span class="col">
          <span class="strip" style="transform:translateY({-Number(d) * 10}%)">
            {#each [0, 1, 2, 3, 4, 5, 6, 7, 8, 9] as n}
              <span class="digit">{n}</span>
            {/each}
          </span>
        </span>
      {:else}
        <span class="sep">{d}</span>
      {/if}
    {/each}
  </span>
</span>

<style>
  .wrap { display: inline-flex; }
  .roll {
    display: inline-flex;
    line-height: 1.12;
    overflow: hidden;
  }
  .col { display: inline-block; height: 1.12em; overflow: hidden; clip-path: inset(0); }
  .strip {
    display: flex;
    flex-direction: column;
    transition: transform 0.7s cubic-bezier(0.2, 1.3, 0.35, 1);
  }
  .digit { height: 1.12em; display: block; text-align: center; }
  .sep { display: inline-block; width: 0.28em; text-align: center; }
  .gold .digit, .gold .sep {
    background: linear-gradient(180deg, #fff6c2 12%, #ffc534 55%, #f08c0c 92%);
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }
</style>
