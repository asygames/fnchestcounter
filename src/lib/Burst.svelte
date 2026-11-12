<script lang="ts">
  // Explozie de monede și steluțe la fiecare cufăr. Canvas-ul desenează doar
  // cât timp există particule; în rest nu consumă nimic.
  let { pulse = 0 }: { pulse?: number } = $props();

  let canvas: HTMLCanvasElement;
  let last = 0;
  type P = { x: number; y: number; vx: number; vy: number; r: number; rot: number; vr: number; life: number; kind: 0 | 1; hue: number };
  let parts: P[] = [];
  let raf = 0;

  $effect(() => {
    if (pulse !== last) {
      last = pulse;
      if (pulse > 0) spawn();
    }
  });

  function spawn() {
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    const cx = rect.width / 2;
    const cy = rect.height * 0.55;
    for (let i = 0; i < 46; i++) {
      const a = -Math.PI / 2 + (Math.random() - 0.5) * 2.2;
      const sp = 5 + Math.random() * 9;
      parts.push({
        x: cx + (Math.random() - 0.5) * 60,
        y: cy,
        vx: Math.cos(a) * sp,
        vy: Math.sin(a) * sp,
        r: 6 + Math.random() * 7,
        rot: Math.random() * 6.28,
        vr: (Math.random() - 0.5) * 0.4,
        life: 1,
        kind: Math.random() < 0.7 ? 0 : 1,
        hue: 38 + Math.random() * 14,
      });
    }
    if (!raf) raf = requestAnimationFrame(tick);
  }

  function star(ctx: CanvasRenderingContext2D, r: number) {
    ctx.beginPath();
    for (let i = 0; i < 8; i++) {
      const rr = i % 2 === 0 ? r : r * 0.4;
      const a = (i * Math.PI) / 4;
      ctx.lineTo(Math.cos(a) * rr, Math.sin(a) * rr);
    }
    ctx.closePath();
  }

  function tick() {
    const ctx = canvas?.getContext("2d");
    if (!ctx) { raf = 0; return; }
    const dpr = canvas.width / Math.max(1, canvas.getBoundingClientRect().width);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    parts = parts.filter((p) => p.life > 0);
    for (const p of parts) {
      p.vy += 0.38;
      p.vx *= 0.985;
      p.x += p.vx;
      p.y += p.vy;
      p.rot += p.vr;
      p.life -= 0.012;
      ctx.save();
      ctx.globalAlpha = Math.min(1, p.life * 2);
      ctx.translate(p.x, p.y);
      ctx.rotate(p.rot);
      ctx.lineWidth = 2.5;
      ctx.strokeStyle = "#2a1208";
      if (p.kind === 0) {
        // monedă văzută din lateral (se „rotește” prin scalare)
        ctx.scale(Math.abs(Math.cos(p.rot * 2)) * 0.8 + 0.2, 1);
        ctx.beginPath();
        ctx.arc(0, 0, p.r, 0, Math.PI * 2);
        ctx.fillStyle = `hsl(${p.hue} 100% 58%)`;
        ctx.fill();
        ctx.stroke();
        ctx.beginPath();
        ctx.arc(0, 0, p.r * 0.55, 0, Math.PI * 2);
        ctx.strokeStyle = "rgba(160, 80, 0, 0.7)";
        ctx.lineWidth = 2;
        ctx.stroke();
      } else {
        star(ctx, p.r * 1.1);
        ctx.fillStyle = "#fffbe6";
        ctx.fill();
        ctx.stroke();
      }
      ctx.restore();
    }
    if (parts.length) raf = requestAnimationFrame(tick);
    else {
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      raf = 0;
    }
  }
</script>

<canvas bind:this={canvas} class="burst"></canvas>

<style>
  .burst { position: absolute; inset: -40px; width: calc(100% + 80px); height: calc(100% + 80px); pointer-events: none; z-index: 5; }
</style>
