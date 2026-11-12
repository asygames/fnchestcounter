<script lang="ts">
  // Cufăr cartoon desenat în SVG. La fiecare `pulse` nou: capacul se deschide,
  // iese lumină aurie și cufărul sare puțin, apoi se închide la loc.
  let { pulse = 0, size = 200 }: { pulse?: number; size?: number } = $props();

  let opening = $state(false);
  let last = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    if (pulse !== last) {
      last = pulse;
      opening = false;
      // repornim animația chiar dacă vine alt cufăr în timpul ei
      requestAnimationFrame(() => {
        opening = true;
        clearTimeout(timer);
        timer = setTimeout(() => (opening = false), 1300);
      });
    }
  });
</script>

<div class="chest" class:opening style="width:{size}px;height:{size}px">
  <svg viewBox="0 0 512 512" aria-hidden="true">
    <defs>
      <linearGradient id="c-wood" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stop-color="#c8742f" />
        <stop offset="1" stop-color="#8a4318" />
      </linearGradient>
      <linearGradient id="c-lid" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stop-color="#dc8a3e" />
        <stop offset="1" stop-color="#a5541f" />
      </linearGradient>
      <linearGradient id="c-gold" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stop-color="#fff0a0" />
        <stop offset="0.45" stop-color="#ffc534" />
        <stop offset="1" stop-color="#e08a0c" />
      </linearGradient>
      <linearGradient id="c-beam" x1="0" y1="1" x2="0" y2="0">
        <stop offset="0" stop-color="#ffe680" stop-opacity="0.95" />
        <stop offset="1" stop-color="#ffe680" stop-opacity="0" />
      </linearGradient>
      <clipPath id="c-lidclip">
        <path d="M90 250 V196 C90 126 154 92 256 92 C358 92 422 126 422 196 V250 Z" />
      </clipPath>
      <clipPath id="c-bodyclip">
        <rect x="96" y="246" width="320" height="176" rx="22" />
      </clipPath>
    </defs>

    <!-- umbra -->
    <ellipse class="shadow" cx="256" cy="452" rx="170" ry="22" fill="#000" opacity="0.35" />

    <g class="jump">
      <!-- raze de lumină care ies din cufăr -->
      <g class="beams">
        <path d="M150 250 L60 20 L200 20 Z" fill="url(#c-beam)" />
        <path d="M220 250 L210 0 L300 0 L292 250 Z" fill="url(#c-beam)" />
        <path d="M362 250 L312 20 L452 20 Z" fill="url(#c-beam)" />
      </g>

      <g stroke="#2a1208" stroke-width="16" stroke-linejoin="round">
        <!-- interiorul (se vede când capacul e deschis) -->
        <rect x="104" y="226" width="304" height="40" rx="10" fill="#ffd34d" stroke-width="12" />
        <!-- corp -->
        <rect x="96" y="246" width="320" height="176" rx="22" fill="url(#c-wood)" />
        <path d="M104 304 H408 M104 362 H408" stroke="#6b3112" stroke-width="8" fill="none" />
        <g clip-path="url(#c-bodyclip)" stroke="none">
          <rect x="132" y="240" width="44" height="190" fill="url(#c-gold)" />
          <rect x="336" y="240" width="44" height="190" fill="url(#c-gold)" />
        </g>
        <path d="M132 250 V422 M176 250 V422 M336 250 V422 M380 250 V422" fill="none" stroke-width="12" />
        <rect x="96" y="246" width="320" height="176" rx="22" fill="none" />
      </g>

      <!-- capacul se rotește în jurul balamalei din spate -->
      <g class="lid">
        <g stroke="#2a1208" stroke-width="16" stroke-linejoin="round">
          <path d="M90 250 V196 C90 126 154 92 256 92 C358 92 422 126 422 196 V250 Z" fill="url(#c-lid)" />
          <g clip-path="url(#c-lidclip)" stroke="none">
            <rect x="132" y="60" width="44" height="200" fill="url(#c-gold)" />
            <rect x="336" y="60" width="44" height="200" fill="url(#c-gold)" />
          </g>
          <path d="M132 120 V250 M176 104 V250 M336 104 V250 M380 120 V250" fill="none" stroke-width="12" />
          <path d="M90 250 V196 C90 126 154 92 256 92 C358 92 422 126 422 196 V250 Z" fill="none" />
          <rect x="84" y="234" width="344" height="36" rx="12" fill="url(#c-gold)" />
          <rect x="212" y="222" width="88" height="100" rx="20" fill="url(#c-gold)" />
          <path d="M256 252 a14 14 0 0 1 9 25 l5 22 h-28 l5 -22 a14 14 0 0 1 9 -25 z" fill="#2a1208" stroke="none" />
        </g>
        <path d="M164 150 q6 -16 22 -18" stroke="#fff6c8" stroke-width="10" stroke-linecap="round" fill="none" />
      </g>

      <!-- scântei -->
      <g class="sparks" fill="#fffbe6" stroke="#2a1208" stroke-width="7" stroke-linejoin="round">
        <path class="s1" d="M430 74 l12 30 30 12 -30 12 -12 30 -12 -30 -30 -12 30 -12 z" />
        <path class="s2" d="M70 92 l8 19 19 8 -19 8 -8 19 -8 -19 -19 -8 19 -8 z" />
        <path class="s3" d="M256 10 l9 22 22 9 -22 9 -9 22 -9 -22 -22 -9 22 -9 z" />
      </g>
    </g>
  </svg>
</div>

<style>
  .chest { position: relative; flex: none; }
  svg { width: 100%; height: 100%; overflow: visible; }

  .lid {
    transform-box: view-box;
    transform-origin: 256px 250px;
    transition: transform 0.45s cubic-bezier(0.5, 1.8, 0.5, 1);
  }
  .beams { opacity: 0; transform-box: view-box; transform-origin: 256px 250px; transform: scaleY(0.2); transition: opacity 0.3s, transform 0.4s; }
  .jump { transform-box: view-box; transform-origin: 256px 452px; }
  .sparks path { transform-box: fill-box; transform-origin: center; animation: twinkle 2.6s ease-in-out infinite; }
  .sparks .s2 { animation-delay: -0.9s; }
  .sparks .s3 { opacity: 0; animation: none; }

  .opening .lid { transform: translateY(-70px) rotate(-14deg); }
  .opening .beams { opacity: 1; transform: scaleY(1); }
  .opening .jump { animation: hop 0.6s cubic-bezier(0.3, 1.6, 0.5, 1); }
  .opening .sparks .s3 { animation: pop 0.9s ease-out; }

  @keyframes hop {
    0% { transform: scale(1, 1); }
    25% { transform: scale(1.12, 0.86); }
    55% { transform: scale(0.94, 1.1) translateY(-18px); }
    100% { transform: scale(1, 1); }
  }
  @keyframes twinkle {
    0%, 100% { transform: scale(1) rotate(0deg); }
    50% { transform: scale(0.72) rotate(20deg); }
  }
  @keyframes pop {
    0% { opacity: 0; transform: scale(0.2) translateY(60px); }
    40% { opacity: 1; transform: scale(1.3) translateY(0); }
    100% { opacity: 0; transform: scale(0.8) translateY(-40px); }
  }
</style>
