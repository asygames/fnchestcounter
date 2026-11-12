//! Comenzile apelate din interfață, scurtăturile globale și overlay-ul.

use crate::detect::fusion::Source;
use crate::engine::{AudioCmd, Engine, Live};
use crate::store::{ChestEvent, Session, Settings};
use serde::Serialize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

type Eng<'a> = State<'a, Arc<Engine>>;

#[derive(Serialize)]
pub struct CalibInfo {
    visual_ready: bool,
    visual_samples: usize,
    visual_score: f32,
    visual_threshold: f32,
    audio_ready: bool,
    audio_samples: u32,
    audio_score: f32,
}

#[derive(Serialize)]
pub struct Snapshot {
    total: i64,
    session: i64,
    session_start: i64,
    today: i64,
    best_day: Option<(String, i64)>,
    days: Vec<(String, i64)>,
    events: Vec<ChestEvent>,
    sessions: Vec<Session>,
    settings: Settings,
    calib: CalibInfo,
    live: Live,
    supported: bool,
    version: &'static str,
    data_dir: String,
}

fn calib_info(e: &Engine) -> CalibInfo {
    let st = e.store.lock();
    let v = e.visual.read();
    CalibInfo {
        visual_ready: v.tpl.is_some(),
        visual_samples: st.calib.visual_samples.len(),
        visual_score: v.tpl.as_ref().map_or(0.0, |t| t.calib_score),
        visual_threshold: v.threshold,
        audio_ready: st.calib.audio.is_some(),
        audio_samples: e.audio_samples.load(Ordering::Relaxed),
        audio_score: st.calib.audio.as_ref().map_or(0.0, |t| t.calib_score),
    }
}

#[tauri::command]
pub fn get_snapshot(engine: Eng) -> Snapshot {
    let calib = calib_info(&engine);
    let live = engine.live.lock().clone();
    let st = engine.store.lock();
    let best_day = st.data.days.iter().max_by_key(|(_, v)| **v).map(|(k, v)| (k.clone(), *v));
    let days: Vec<(String, i64)> = st.data.days.iter().rev().take(120).map(|(k, v)| (k.clone(), *v)).collect();
    Snapshot {
        total: st.data.total,
        session: st.session.count,
        session_start: st.session.start,
        today: st.today(),
        best_day,
        days,
        events: st.data.events.iter().rev().take(60).cloned().collect(),
        sessions: st.data.sessions.iter().rev().take(40).cloned().collect(),
        settings: st.settings.clone(),
        calib,
        live,
        supported: crate::platform::SUPPORTED,
        version: env!("CARGO_PKG_VERSION"),
        data_dir: st.dir().display().to_string(),
    }
}

#[tauri::command]
pub fn get_live(engine: Eng) -> Live {
    engine.live.lock().clone()
}

#[tauri::command]
pub fn add_manual(engine: Eng, delta: i32) -> bool {
    engine.count(delta.signum(), Source::Manual)
}

pub fn new_session_inner(app: &AppHandle) {
    let e = app.state::<Arc<Engine>>();
    e.store.lock().new_session();
    let _ = app.emit("refresh", ());
    crate::tray::update_tooltip(app, 0, e.store.lock().today());
}

#[tauri::command]
pub fn new_session(app: AppHandle) {
    new_session_inner(&app);
}

#[tauri::command]
pub fn reset_all(app: AppHandle, engine: Eng) {
    engine.store.lock().reset_all();
    let _ = app.emit("refresh", ());
    crate::tray::update_tooltip(&app, 0, 0);
}

#[tauri::command]
pub fn update_settings(app: AppHandle, engine: Eng, settings: Settings) -> Result<Settings, String> {
    // Verificăm scurtăturile înainte să salvăm.
    for hk in [&settings.hotkey_add, &settings.hotkey_sub, &settings.hotkey_visual_sample, &settings.hotkey_audio_sample] {
        if !hk.is_empty() {
            hk.parse::<Shortcut>().map_err(|e| format!("Scurtătură invalidă „{hk}”: {e}"))?;
        }
    }
    let mut s = settings;
    s.fps = s.fps.clamp(5, 30);
    s.roi = s.roi.sanitized();
    s.overlay_scale = s.overlay_scale.clamp(0.6, 2.0);
    s.visual_threshold = s.visual_threshold.map(|t| t.clamp(0.3, 0.95));
    s.audio_threshold = s.audio_threshold.map(|t| t.clamp(0.3, 0.95));
    let prev = {
        let mut st = engine.store.lock();
        let prev = std::mem::replace(&mut st.settings, s.clone());
        st.save_settings();
        prev
    };
    engine.apply_settings(&s, &prev);
    if prev.overlay != s.overlay || prev.overlay_scale != s.overlay_scale {
        apply_overlay(&app, &s);
    }
    register_hotkeys(&app, &s);
    let _ = app.emit("settings", &s);
    Ok(s)
}

#[tauri::command]
pub fn visual_take_sample(engine: Eng) -> Result<(), String> {
    let cap = engine.live.lock().capture.clone();
    if cap != "capturing" {
        return Err("Nu văd jocul încă. Pornește Fortnite (sau alege monitorul în Setări).".into());
    }
    engine.visual_sample_req.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub fn visual_clear(engine: Eng) {
    engine.clear_visual();
}

#[tauri::command]
pub fn audio_take_sample(engine: Eng) -> Result<(), String> {
    if engine.live.lock().audio != "listening" {
        return Err("Nu ascult sunetul încă (verifică în Setări că detecția audio e pornită).".into());
    }
    engine.audio_cmds.lock().push_back(AudioCmd::TakeSample);
    Ok(())
}

#[tauri::command]
pub fn audio_clear(engine: Eng) {
    {
        let mut st = engine.store.lock();
        st.calib.audio = None;
        st.save_calib();
    }
    engine.audio_cmds.lock().push_back(AudioCmd::Clear);
}

/// Previzualizarea zonei analizate (binar: w, h, has_tpl, cur, [mean, mask]).
#[tauri::command]
pub fn visual_preview(engine: Eng) -> Response {
    Response::new(engine.visual_preview())
}

/// Un cadru micșorat din joc, ca să poți alege zona (binar: w, h, RGBA).
#[tauri::command]
pub async fn get_thumbnail(engine: State<'_, Arc<Engine>>) -> Result<Response, String> {
    if engine.live.lock().capture != "capturing" {
        return Err("Nu văd jocul încă.".into());
    }
    *engine.thumb.lock() = None;
    engine.thumb_req.store(true, Ordering::Relaxed);
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_millis(2500) {
        if let Some(t) = engine.thumb.lock().take() {
            let mut out = Vec::with_capacity(8 + t.rgba.len());
            out.extend_from_slice(&t.w.to_le_bytes());
            out.extend_from_slice(&t.h.to_le_bytes());
            out.extend_from_slice(&t.rgba);
            return Ok(Response::new(out));
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    engine.thumb_req.store(false, Ordering::Relaxed);
    Err("Jocul nu a trimis niciun cadru (e minimizat?).".into())
}

#[tauri::command]
pub fn overlay_edit(app: AppHandle, editing: bool) {
    if let Some(w) = app.get_webview_window("overlay") {
        if !w.is_visible().unwrap_or(false) {
            return;
        }
        let _ = w.set_ignore_cursor_events(!editing);
        let _ = app.emit_to("overlay", "overlay-edit", editing);
        if !editing {
            if let Ok(pos) = w.outer_position() {
                let e = app.state::<Arc<Engine>>();
                let mut st = e.store.lock();
                st.settings.overlay_x = Some(pos.x);
                st.settings.overlay_y = Some(pos.y);
                st.save_settings();
            }
        }
    }
}

#[cfg(debug_assertions)]
#[tauri::command]
pub fn simulate_chest(engine: Eng, source: Option<String>) {
    let src = match source.as_deref() {
        Some("audio") => Source::Audio,
        Some("both") => Source::Both,
        _ => Source::Visual,
    };
    engine.count(1, src);
}

#[cfg(not(debug_assertions))]
#[tauri::command]
pub fn simulate_chest() {}

pub fn toggle_overlay(app: &AppHandle) {
    let e = app.state::<Arc<Engine>>();
    let s = {
        let mut st = e.store.lock();
        st.settings.overlay = !st.settings.overlay;
        st.save_settings();
        st.settings.clone()
    };
    apply_overlay(app, &s);
    let _ = app.emit("settings", &s);
}

pub fn apply_overlay(app: &AppHandle, s: &Settings) {
    let Some(w) = app.get_webview_window("overlay") else { return };
    if !s.overlay {
        let _ = w.hide();
        return;
    }
    let scale = s.overlay_scale;
    let _ = w.set_size(tauri::LogicalSize::new(250.0 * scale as f64, 96.0 * scale as f64));
    match (s.overlay_x, s.overlay_y) {
        (Some(x), Some(y)) => {
            let _ = w.set_position(PhysicalPosition::new(x, y));
        }
        _ => {
            // Implicit: sus în dreapta monitorului principal.
            if let Ok(Some(m)) = w.primary_monitor() {
                let size = m.size();
                let ws = w.outer_size().map(|s| s.width).unwrap_or(260);
                let x = m.position().x + size.width as i32 - ws as i32 - (24.0 * m.scale_factor()) as i32;
                let y = m.position().y + (110.0 * m.scale_factor()) as i32;
                let _ = w.set_position(PhysicalPosition::new(x, y));
            }
        }
    }
    // Întâi afișăm fereastra: pe unele sisteme click-through-ul cere o fereastră deja creată.
    let _ = w.show();
    let _ = w.set_always_on_top(true);
    let _ = w.set_ignore_cursor_events(true);
}

pub fn register_hotkeys(app: &AppHandle, s: &Settings) {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let bindings: [(&str, &'static str); 4] = [
        (&s.hotkey_add, "add"),
        (&s.hotkey_sub, "sub"),
        (&s.hotkey_visual_sample, "visual"),
        (&s.hotkey_audio_sample, "audio"),
    ];
    for (key, action) in bindings {
        if key.is_empty() {
            continue;
        }
        let Ok(sc) = key.parse::<Shortcut>() else { continue };
        let res = gs.on_shortcut(sc, move |app, _sc, ev| {
            if ev.state() != ShortcutState::Pressed {
                return;
            }
            let e = app.state::<Arc<Engine>>();
            match action {
                "add" => {
                    e.count(1, Source::Manual);
                }
                "sub" => {
                    e.count(-1, Source::Manual);
                }
                "visual" => {
                    if e.live.lock().capture == "capturing" {
                        e.visual_sample_req.store(true, Ordering::Relaxed);
                    } else {
                        e.emit_calib(crate::engine::CalibPayload {
                            kind: "visual",
                            ok: false,
                            message: "Nu văd jocul încă.".into(),
                            samples: 0,
                            ready: false,
                            score: 0.0,
                        });
                    }
                }
                "audio" => e.audio_cmds.lock().push_back(AudioCmd::TakeSample),
                _ => {}
            }
        });
        if let Err(err) = res {
            log::warn!("nu pot înregistra {key}: {err}");
            let _ = app.emit("hotkey-error", format!("Tasta {key} e deja folosită de alt program."));
        }
    }
}

pub fn quit(app: &AppHandle) {
    let e = app.state::<Arc<Engine>>();
    e.shutdown.store(true, Ordering::Relaxed);
    e.store.lock().save_data();
    app.exit(0);
}
