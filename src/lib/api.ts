// Legătura cu backend-ul Rust. În browser (fără Tauri) folosim date demo,
// ca interfața să poată fi dezvoltată și verificată și în afara aplicației.

import type { UnlistenFn } from "@tauri-apps/api/event";

export type Source = "visual" | "audio" | "both" | "manual";
export type Mode = "either" | "both" | "visual" | "audio";

export interface Roi { x0: number; y0: number; x1: number; y1: number }

export interface Settings {
  mode: Mode;
  roi: Roi;
  fps: number;
  visual_threshold: number | null;
  audio_threshold: number | null;
  audio_enabled: boolean;
  visual_enabled: boolean;
  interact_vk: number;
  interact_label: string;
  gamepad: boolean;
  require_key: boolean;
  capture_source: "window" | "monitor";
  monitor_index: number;
  overlay: boolean;
  overlay_x: number | null;
  overlay_y: number | null;
  overlay_scale: number;
  close_to_tray: boolean;
  sounds: boolean;
  hotkey_add: string;
  hotkey_sub: string;
  hotkey_visual_sample: string;
  hotkey_audio_sample: string;
  reduce_motion: boolean;
}

export interface Live {
  capture: "unsupported" | "waiting" | "capturing" | "disabled" | "error";
  capture_target: string;
  capture_error: string;
  frame_w: number;
  frame_h: number;
  fps: number;
  analyze_ms: number;
  visual_score: number;
  prompt_visible: boolean;
  audio: "unsupported" | "off" | "listening" | "error";
  audio_device: string;
  audio_score: number;
  key_down: boolean;
}

export interface ChestEvent { ts: number; source: Source; delta: number }
export interface Session { id: number; start: number; end: number; count: number }

export interface CalibInfo {
  visual_ready: boolean;
  visual_samples: number;
  visual_score: number;
  visual_threshold: number;
  audio_ready: boolean;
  audio_samples: number;
  audio_score: number;
}

export interface Snapshot {
  total: number;
  session: number;
  session_start: number;
  today: number;
  best_day: [string, number] | null;
  days: [string, number][];
  events: ChestEvent[];
  sessions: Session[];
  settings: Settings;
  calib: CalibInfo;
  live: Live;
  supported: boolean;
  version: string;
  data_dir: string;
}

export interface ChestPayload { total: number; session: number; today: number; delta: number; source: Source; ts: number }
export interface CalibPayload { kind: "visual" | "audio"; ok: boolean; message: string; samples: number; ready: boolean; score: number }

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function tauriInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

export async function listen<T>(event: string, cb: (payload: T) => void): Promise<UnlistenFn> {
  if (!isTauri) return mock.listen(event, cb as (p: unknown) => void);
  const { listen } = await import("@tauri-apps/api/event");
  return listen<T>(event, (e) => cb(e.payload));
}

export const api = {
  snapshot: () => (isTauri ? tauriInvoke<Snapshot>("get_snapshot") : mock.snapshot()),
  addManual: (delta: number) => (isTauri ? tauriInvoke<boolean>("add_manual", { delta }) : mock.add(delta, "manual")),
  newSession: () => (isTauri ? tauriInvoke<void>("new_session") : mock.newSession()),
  resetAll: () => (isTauri ? tauriInvoke<void>("reset_all") : mock.resetAll()),
  updateSettings: (settings: Settings) =>
    isTauri ? tauriInvoke<Settings>("update_settings", { settings }) : mock.updateSettings(settings),
  visualSample: () => (isTauri ? tauriInvoke<void>("visual_take_sample") : mock.visualSample()),
  visualClear: () => (isTauri ? tauriInvoke<void>("visual_clear") : mock.visualClear()),
  audioSample: () => (isTauri ? tauriInvoke<void>("audio_take_sample") : mock.audioSample()),
  audioClear: () => (isTauri ? tauriInvoke<void>("audio_clear") : mock.audioClear()),
  visualPreview: async (): Promise<ArrayBuffer> =>
    isTauri ? tauriInvoke<ArrayBuffer>("visual_preview") : mock.visualPreview(),
  thumbnail: async (): Promise<ArrayBuffer> => (isTauri ? tauriInvoke<ArrayBuffer>("get_thumbnail") : mock.thumbnail()),
  overlayEdit: (editing: boolean) => (isTauri ? tauriInvoke<void>("overlay_edit", { editing }) : Promise.resolve()),
  simulate: (source?: Source) => (isTauri ? tauriInvoke<void>("simulate_chest", { source }) : mock.add(1, source ?? "visual")),
};

export async function windowAction(action: "minimize" | "maximize" | "close") {
  if (!isTauri) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const w = getCurrentWindow();
  if (action === "minimize") await w.minimize();
  else if (action === "maximize") await w.toggleMaximize();
  else await w.close();
}

// ───────────────────────────── Demo (browser) ─────────────────────────────

const mock = (() => {
  const listeners = new Map<string, Set<(p: unknown) => void>>();
  const emit = (ev: string, p: unknown) => listeners.get(ev)?.forEach((cb) => cb(p));
  const day = (offset: number) => {
    const d = new Date();
    d.setDate(d.getDate() - offset);
    return d.toISOString().slice(0, 10);
  };
  const now = Date.now();
  const days: [string, number][] = [];
  for (let i = 0; i < 40; i++) {
    const v = i === 0 ? 34 : Math.max(0, Math.round(28 + 22 * Math.sin(i * 1.7) + (i % 5) * 3 - (i % 7 === 3 ? 30 : 0)));
    if (v > 0) days.push([day(i), v]);
  }
  const snap: Snapshot = {
    total: 4182,
    session: 12,
    session_start: now - 72 * 60_000,
    today: 34,
    best_day: [day(9), 71],
    days,
    events: Array.from({ length: 12 }, (_, i) => ({
      ts: now - i * 5.5 * 60_000 - 40_000,
      source: (["visual", "both", "audio", "visual", "manual"] as Source[])[i % 5],
      delta: 1,
    })),
    sessions: Array.from({ length: 8 }, (_, i) => ({
      id: i,
      start: now - (i + 1) * 26 * 3600_000,
      end: now - (i + 1) * 26 * 3600_000 + (50 + i * 13) * 60_000,
      count: 8 + ((i * 7) % 19),
    })),
    settings: {
      mode: "either",
      roi: { x0: 0.38, y0: 0.5, x1: 0.7, y1: 0.72 },
      fps: 15,
      visual_threshold: null,
      audio_threshold: null,
      audio_enabled: true,
      visual_enabled: true,
      interact_vk: 0x45,
      interact_label: "E",
      gamepad: true,
      require_key: true,
      capture_source: "window",
      monitor_index: 0,
      overlay: true,
      overlay_x: null,
      overlay_y: null,
      overlay_scale: 1,
      close_to_tray: true,
      sounds: false,
      hotkey_add: "F9",
      hotkey_sub: "F10",
      hotkey_visual_sample: "F8",
      hotkey_audio_sample: "F7",
      reduce_motion: false,
    },
    calib: { visual_ready: true, visual_samples: 4, visual_score: 0.91, visual_threshold: 0.66, audio_ready: false, audio_samples: 0, audio_score: 0 },
    live: {
      capture: "capturing", capture_target: "Fortnite", capture_error: "", frame_w: 2560, frame_h: 1440,
      fps: 15, analyze_ms: 0.9, visual_score: 0.12, prompt_visible: false,
      audio: "listening", audio_device: "Căști (Realtek)", audio_score: 0.08, key_down: false,
    },
    supported: true,
    version: "0.1.0-demo",
    data_dir: "C:\\Users\\Jucator\\AppData\\Roaming\\com.asygames.fnchestcounter",
  };

  let t = 0;
  setInterval(() => {
    t++;
    const l = snap.live;
    const phase = t % 40;
    l.prompt_visible = phase > 10 && phase < 22;
    l.visual_score = l.prompt_visible ? 0.82 + Math.random() * 0.1 : 0.05 + Math.random() * 0.15;
    l.audio_score = phase === 22 || phase === 23 ? 0.88 : Math.random() * 0.2;
    l.key_down = phase > 17 && phase < 22;
    l.fps = 14.6 + Math.random() * 0.6;
    l.analyze_ms = 0.7 + Math.random() * 0.4;
    emit("live", { ...l });
  }, 200);

  return {
    listen(event: string, cb: (p: unknown) => void): UnlistenFn {
      if (!listeners.has(event)) listeners.set(event, new Set());
      listeners.get(event)!.add(cb);
      return () => listeners.get(event)?.delete(cb);
    },
    async snapshot() {
      return structuredClone(snap);
    },
    async add(delta: number, source: Source) {
      if (delta < 0 && snap.session <= 0) return false;
      snap.total += delta;
      snap.session += delta;
      snap.today += delta;
      if (snap.days[0]) snap.days[0][1] = snap.today;
      const ev = { ts: Date.now(), source, delta };
      snap.events.unshift(ev);
      emit("chest", { total: snap.total, session: snap.session, today: snap.today, delta, source, ts: ev.ts });
      return true;
    },
    async newSession() {
      snap.session = 0;
      snap.session_start = Date.now();
      emit("refresh", null);
    },
    async resetAll() {
      snap.total = 0; snap.session = 0; snap.today = 0; snap.days = []; snap.events = []; snap.sessions = []; snap.best_day = null;
      emit("refresh", null);
    },
    async updateSettings(s: Settings) {
      snap.settings = structuredClone(s);
      emit("settings", s);
      return s;
    },
    async visualSample() {
      const n = Math.min(snap.calib.visual_samples + 1, 8);
      snap.calib.visual_samples = n;
      setTimeout(() => emit("calib", { kind: "visual", ok: true, message: n < 3 ? `Proba ${n} salvată. Mai fă ${3 - n} la alt cufăr, în alt loc.` : "Detecția vizuală e gata (potrivire 91%).", samples: n, ready: n >= 3, score: 0.91 }), 150);
    },
    async visualClear() { snap.calib.visual_samples = 0; snap.calib.visual_ready = false; },
    async audioSample() {
      const n = snap.calib.audio_samples + 1;
      snap.calib.audio_samples = n >= 2 ? 0 : n;
      if (n >= 2) snap.calib.audio_ready = true;
      setTimeout(() => emit("calib", { kind: "audio", ok: true, message: n >= 2 ? "Sunetul cufărului e învățat (potrivire 88%)." : "Proba 1 de sunet salvată. Deschide încă un cufăr și apasă din nou.", samples: n, ready: n >= 2, score: 0.88 }), 150);
    },
    async audioClear() { snap.calib.audio_ready = false; snap.calib.audio_samples = 0; },
    async visualPreview(): Promise<ArrayBuffer> {
      const w = 192, h = 85;
      const buf = new Uint8Array(9 + w * h * 3);
      const dv = new DataView(buf.buffer);
      dv.setUint32(0, w, true); dv.setUint32(4, h, true); buf[8] = 1;
      for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
        const i = y * w + x;
        const ui = y > 30 && y < 52 && x > 50 && x < 140;
        const text = ui && y > 38 && y < 45 && ((x > 74 && x < 90) || (x > 94 && x < 102) || (x > 106 && x < 134));
        const key = ui && x > 54 && x < 68 && y > 34 && y < 48;
        const bg = 70 + 40 * Math.sin(x / 13) + 30 * Math.cos(y / 9);
        const v = text ? 245 : key ? 210 : ui ? 30 : bg;
        buf[9 + i] = v;
        buf[9 + w * h + i] = ui ? v : 90;
        buf[9 + 2 * w * h + i] = text || key ? 255 : 0;
      }
      return buf.buffer;
    },
    async thumbnail(): Promise<ArrayBuffer> {
      const w = 480, h = 270;
      const buf = new Uint8Array(8 + w * h * 4);
      const dv = new DataView(buf.buffer);
      dv.setUint32(0, w, true); dv.setUint32(4, h, true);
      for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
        const o = 8 + (y * w + x) * 4;
        const sky = y < h * 0.45;
        buf[o] = sky ? 120 + y / 3 : 70 + (x % 40);
        buf[o + 1] = sky ? 170 + y / 4 : 120 + ((x + y) % 30);
        buf[o + 2] = sky ? 230 : 60;
        buf[o + 3] = 255;
      }
      return buf.buffer;
    },
  };
})();
