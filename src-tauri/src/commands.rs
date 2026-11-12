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
    /// Detectorul automat (fără calibrare) e pornit.
    visual_auto: bool,
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
    let auto = v.auto();
    CalibInfo {
        visual_auto: auto,
        visual_ready: auto || v.tpl.is_some(),
        visual_samples: st.calib.visual_samples.len(),
        visual_score: v.tpl.as_ref().map_or(0.0, |t| t.calib_score),
        visual_threshold: if auto { 0.75 } else { v.threshold },
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
    for hk in [
        &settings.hotkey_add,
        &settings.hotkey_sub,
        &settings.hotkey_visual_sample,
        &settings.hotkey_audio_sample,
        &settings.hotkey_overlay_move,
    ] {
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
    s.search_words = s.search_words.trim().chars().take(200).collect();
    if crate::detect::prompt::parse_words(&s.search_words).is_empty() {
        s.search_words = "SEARCH".into();
    }
    let prev = {
        let mut st = engine.store.lock();
        let prev = std::mem::replace(&mut st.settings, s.clone());
        st.save_settings();
        prev
    };
    engine.apply_settings(&s, &prev);
    if prev.overlay != s.overlay
        || prev.overlay_scale != s.overlay_scale
        || prev.overlay_x != s.overlay_x
        || prev.overlay_y != s.overlay_y
    {
        apply_overlay(&app, &s);
    }
    register_hotkeys(&app, &s);
    let _ = app.emit("settings", &s);
    Ok(s)
}

#[tauri::command]
pub fn visual_take_sample(engine: Eng) -> Result<(), String> {
    if engine.visual.read().auto() {
        return Err("Detecția automată nu are nevoie de probe. Probele sunt doar pentru modul manual.".into());
    }
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
    set_overlay_edit(&app, editing);
}

/// Pornește/oprește modul „mută overlay-ul”. Merge din Setări, din butonul „Gata”
/// de pe overlay și din tasta globală (implicit F6), deci și din joc.
pub fn set_overlay_edit(app: &AppHandle, editing: bool) {
    let Some(w) = app.get_webview_window("overlay") else { return };
    let e = app.state::<Arc<Engine>>();
    e.overlay_editing.store(editing, Ordering::Relaxed);
    if editing {
        // Dacă overlay-ul era ascuns, îl arătăm: altfel nu ai ce muta.
        let shown = {
            let mut st = e.store.lock();
            let was_off = !st.settings.overlay;
            if was_off {
                st.settings.overlay = true;
                st.save_settings();
            }
            was_off.then(|| st.settings.clone())
        };
        if let Some(s) = shown {
            apply_overlay(app, &s);
            let _ = app.emit("settings", &s);
        }
    }
    let _ = w.set_ignore_cursor_events(!editing);
    let _ = app.emit("overlay-edit", editing);
    if !editing {
        if let Ok(pos) = w.outer_position() {
            let s = {
                let mut st = e.store.lock();
                st.settings.overlay_x = Some(pos.x);
                st.settings.overlay_y = Some(pos.y);
                st.save_settings();
                st.settings.clone()
            };
            // Interfața trebuie să știe noua poziție, altfel o altă setare salvată ar suprascrie-o.
            let _ = app.emit("settings", &s);
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
    // Poziția salvată, dacă încape încă pe un monitor (s-ar fi putut deconecta unul).
    let saved = match (s.overlay_x, s.overlay_y) {
        (Some(x), Some(y)) => w.available_monitors().ok().and_then(|ms| {
            ms.iter()
                .any(|m| {
                    let (p, z) = (m.position(), m.size());
                    x + 40 > p.x && y + 20 > p.y && x < p.x + z.width as i32 - 40 && y < p.y + z.height as i32 - 20
                })
                .then_some((x, y))
        }),
        _ => None,
    };
    match saved {
        Some((x, y)) => {
            let _ = w.set_position(PhysicalPosition::new(x, y));
        }
        None => {
            // Implicit: sus în stânga monitorului principal. În dreapta sus e minimap-ul Fortnite.
            if let Ok(Some(m)) = w.primary_monitor() {
                let x = m.position().x + (24.0 * m.scale_factor()) as i32;
                let y = m.position().y + (90.0 * m.scale_factor()) as i32;
                let _ = w.set_position(PhysicalPosition::new(x, y));
            }
        }
    }
    // Întâi afișăm fereastra: pe unele sisteme click-through-ul cere o fereastră deja creată.
    let _ = w.show();
    let _ = w.set_always_on_top(true);
    let editing = app.state::<Arc<Engine>>().overlay_editing.load(Ordering::Relaxed);
    let _ = w.set_ignore_cursor_events(!editing);
}

pub fn register_hotkeys(app: &AppHandle, s: &Settings) {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let bindings: [(&str, &'static str); 5] = [
        (&s.hotkey_add, "add"),
        (&s.hotkey_sub, "sub"),
        (&s.hotkey_visual_sample, "visual"),
        (&s.hotkey_audio_sample, "audio"),
        (&s.hotkey_overlay_move, "overlay_move"),
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
                    if e.visual.read().auto() {
                        e.emit_calib(crate::engine::CalibPayload {
                            kind: "visual",
                            ok: true,
                            message: "Detecția automată nu are nevoie de probe: citesc singur promptul SEARCH.".into(),
                            samples: 0,
                            ready: true,
                            score: 0.0,
                        });
                    } else if e.live.lock().capture == "capturing" {
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
                "overlay_move" => {
                    let editing = !e.overlay_editing.load(Ordering::Relaxed);
                    set_overlay_edit(app, editing);
                }
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
