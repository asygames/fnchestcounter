<script lang="ts">
  // Iconițe simple desenate ca SVG (contur gros, rotunjit), fără emoji.
  let { name, size = 20 }: { name: string; size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  {#if name === "home"}
    <path d="M3 11.5 12 4l9 7.5" /><path d="M5.5 10v9.5h13V10" /><path d="M10 19.5v-5h4v5" />
  {:else if name === "stats"}
    <path d="M4 20h16" /><rect x="5.5" y="11" width="3" height="6" rx="1" /><rect x="10.5" y="6" width="3" height="11" rx="1" /><rect x="15.5" y="9" width="3" height="8" rx="1" />
  {:else if name === "target"}
    <circle cx="12" cy="12" r="8" /><circle cx="12" cy="12" r="4" /><path d="M12 2v3M12 19v3M2 12h3M19 12h3" />
  {:else if name === "settings"}
    <circle cx="12" cy="12" r="3" /><path d="M12 2.8v2.4M12 18.8v2.4M2.8 12h2.4M18.8 12h2.4M5.5 5.5l1.7 1.7M16.8 16.8l1.7 1.7M5.5 18.5l1.7-1.7M16.8 7.2l1.7-1.7" />
  {:else if name === "eye"}
    <path d="M2 12s3.6-6.5 10-6.5S22 12 22 12s-3.6 6.5-10 6.5S2 12 2 12Z" /><circle cx="12" cy="12" r="3" />
  {:else if name === "ear"}
    <path d="M7 9a5 5 0 0 1 10 0c0 3-2.5 3.6-3 6s-1.5 4-4 4" /><path d="M10 9.5a2 2 0 0 1 4 0" />
  {:else if name === "key"}
    <rect x="3" y="6" width="18" height="12" rx="3" /><path d="M7 10h.01M11 10h.01M15 10h.01M8 14h8" />
  {:else if name === "plus"}
    <path d="M12 5v14M5 12h14" />
  {:else if name === "minus"}
    <path d="M5 12h14" />
  {:else if name === "refresh"}
    <path d="M20 11a8 8 0 1 0-2.3 5.7" /><path d="M20 4v7h-7" />
  {:else if name === "calendar"}
    <rect x="3.5" y="5" width="17" height="15" rx="3" /><path d="M3.5 10h17M8 3v4M16 3v4" />
  {:else if name === "trophy"}
    <path d="M7 4h10v5a5 5 0 0 1-10 0V4Z" /><path d="M7 6H4v1.5A3.5 3.5 0 0 0 7.5 11M17 6h3v1.5a3.5 3.5 0 0 1-3.5 3.5" /><path d="M12 14v3M8.5 20h7M10 17h4" />
  {:else if name === "chest"}
    <path d="M4 11V9a8 4 0 0 1 16 0v2" /><rect x="3" y="11" width="18" height="9" rx="2" /><path d="M10.5 11v3h3v-3" />
  {:else if name === "clock"}
    <circle cx="12" cy="12" r="8.5" /><path d="M12 7.5V12l3 2" />
  {:else if name === "layers"}
    <path d="m12 3 9 5-9 5-9-5 9-5Z" /><path d="m3 13 9 5 9-5" />
  {:else if name === "move"}
    <path d="M12 3v18M3 12h18M12 3l-3 3M12 3l3 3M12 21l-3-3M12 21l3-3M3 12l3-3M3 12l3 3M21 12l-3-3M21 12l-3 3" />
  {:else if name === "check"}
    <path d="m5 12.5 4.5 4.5L19 7.5" />
  {:else if name === "x"}
    <path d="M6 6l12 12M18 6 6 18" />
  {:else if name === "trash"}
    <path d="M4 7h16M9.5 7V4.5h5V7M6 7l1 13h10l1-13" />
  {:else if name === "camera"}
    <path d="M4 8h3l2-3h6l2 3h3v11H4V8Z" /><circle cx="12" cy="13" r="3.5" />
  {:else if name === "bolt"}
    <path d="M13 2 4 14h7l-1 8 9-12h-7l1-8Z" />
  {:else if name === "gamepad"}
    <path d="M7 8h10a4 4 0 0 1 4 4.5l-.6 4a2.5 2.5 0 0 1-4.4 1.2L14.5 16h-5L8 17.7a2.5 2.5 0 0 1-4.4-1.2l-.6-4A4 4 0 0 1 7 8Z" /><path d="M8 11v3M6.5 12.5h3" /><path d="M15.5 12h.01M17.5 13.5h.01" />
  {:else if name === "info"}
    <circle cx="12" cy="12" r="9" /><path d="M12 11v5.5M12 7.5h.01" />
  {:else if name === "window"}
    <rect x="3" y="4.5" width="18" height="15" rx="2.5" /><path d="M3 9h18" />
  {:else if name === "shield"}
    <path d="M12 3 4.5 6v5.5c0 4.5 3.2 8 7.5 9.5 4.3-1.5 7.5-5 7.5-9.5V6L12 3Z" /><path d="m8.8 12 2.2 2.2 4.4-4.4" />
  {/if}
</svg>
