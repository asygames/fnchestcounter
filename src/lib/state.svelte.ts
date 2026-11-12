// Starea globală a interfeței (Svelte 5 runes).

import { api, listen, type CalibPayload, type ChestPayload, type Live, type Settings, type Snapshot } from "./api";
import { playChime } from "./sound";

export type Page = "home" | "stats" | "calibrate" | "settings";

export interface Toast { id: number; kind: "ok" | "err" | "info"; text: string }

class AppState {
  snap = $state<Snapshot | null>(null);
  live = $state<Live | null>(null);
  page = $state<Page>("home");
  toasts = $state<Toast[]>([]);
  /** Crește la fiecare cufăr numărat, pentru animații. */
  pulse = $state(0);
  lastDelta = $state(1);
  lastSource = $state<string>("visual");
  private toastId = 0;

  get settings(): Settings | null {
    return this.snap?.settings ?? null;
  }

  async init() {
    await this.reload();
    await listen<ChestPayload>("chest", (p) => this.onChest(p));
    await listen<Live>("live", (l) => (this.live = l));
    await listen<CalibPayload>("calib", (c) => this.onCalib(c));
    await listen<Settings>("settings", (s) => {
      if (this.snap) this.snap.settings = s;
    });
    await listen<null>("refresh", () => this.reload());
    await listen<string>("hotkey-error", (m) => this.toast("err", m));
  }

  async reload() {
    const s = await api.snapshot();
    this.snap = s;
    this.live = s.live;
  }

  private onChest(p: ChestPayload) {
    const s = this.snap;
    if (!s) return;
    s.total = p.total;
    s.session = p.session;
    s.today = p.today;
    s.events = [{ ts: p.ts, source: p.source, delta: p.delta }, ...s.events].slice(0, 60);
    const key = new Date(p.ts).toLocaleDateString("sv-SE");
    const d = s.days.find((x) => x[0] === key);
    if (d) d[1] = p.today;
    else s.days = [[key, p.today], ...s.days];
    if (!s.best_day || p.today > s.best_day[1]) s.best_day = [key, p.today];
    this.lastDelta = p.delta;
    this.lastSource = p.source;
    if (p.delta > 0) {
      this.pulse++;
      if (s.settings.sounds) playChime();
    }
  }

  private onCalib(c: CalibPayload) {
    const s = this.snap;
    if (s) {
      if (c.kind === "visual") {
        s.calib.visual_samples = c.samples;
        s.calib.visual_ready = c.ready;
        if (c.score > 0) s.calib.visual_score = c.score;
      } else {
        s.calib.audio_samples = c.ready && c.ok && c.score > 0 ? 0 : c.samples;
        s.calib.audio_ready = c.ready;
        if (c.score > 0) s.calib.audio_score = c.score;
      }
    }
    this.toast(c.ok ? "ok" : "err", c.message);
    if (c.ok && c.score > 0) void this.reload();
  }

  toast(kind: Toast["kind"], text: string) {
    const id = ++this.toastId;
    this.toasts = [...this.toasts, { id, kind, text }].slice(-4);
    setTimeout(() => (this.toasts = this.toasts.filter((t) => t.id !== id)), kind === "err" ? 6500 : 4200);
  }

  async saveSettings(patch: Partial<Settings>) {
    if (!this.snap) return;
    const next = { ...this.snap.settings, ...patch };
    try {
      this.snap.settings = await api.updateSettings(next);
    } catch (e) {
      this.toast("err", String(e));
    }
  }
}

export const app = new AppState();

// ───────────────────────────── Utilitare de afișare ─────────────────────────────

export function fmt(n: number): string {
  return n.toLocaleString("ro-RO");
}

export function ago(ts: number, now = Date.now()): string {
  const s = Math.max(0, Math.round((now - ts) / 1000));
  if (s < 10) return "chiar acum";
  if (s < 60) return `acum ${s} s`;
  const m = Math.round(s / 60);
  if (m < 60) return `acum ${m} min`;
  const h = Math.round(m / 60);
  if (h < 24) return h === 1 ? "acum o oră" : `acum ${h} ore`;
  const d = Math.round(h / 24);
  return d === 1 ? "ieri" : `acum ${d} zile`;
}

export function duration(ms: number): string {
  const m = Math.max(0, Math.round(ms / 60000));
  if (m < 60) return `${m} min`;
  const h = Math.floor(m / 60);
  return `${h} h ${m % 60} min`;
}

export const sourceLabel: Record<string, string> = {
  visual: "Imagine",
  audio: "Sunet",
  both: "Imagine + sunet",
  manual: "Manual",
};
