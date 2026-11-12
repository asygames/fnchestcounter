//! Motorul: starea comună dintre firele de captură/sunet/taste și interfață.

use crate::detect::{
    audio::AudioTemplate,
    fusion::{Fusion, Source},
    prompt::{self, OcrImage, OcrTracker, Rgb},
    visual::{self, Gray, Roi, Template},
};
use crate::store::{now_ms, Settings, Store, VisualMethod};
use parking_lot::{Condvar, Mutex, RwLock};
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter};

/// Ce citește firul de captură la fiecare cadru.
#[derive(Clone, Debug)]
pub struct VisualCfg {
    pub enabled: bool,
    pub tpl: Option<Arc<Template>>,
    pub threshold: f32,
    pub roi: Roi,
    pub fps: u32,
    pub require_key: bool,
    pub method: VisualMethod,
    pub chest_only: bool,
    /// Cuvintele promptului, deja normalizate.
    pub words: Arc<Vec<String>>,
}

impl VisualCfg {
    pub fn auto(&self) -> bool {
        self.method == VisualMethod::Auto
    }
}

fn words_of(s: &Settings) -> Arc<Vec<String>> {
    let w = prompt::parse_words(&s.search_words);
    Arc::new(if w.is_empty() { prompt::parse_words("SEARCH") } else { w })
}

pub enum AudioCmd {
    TakeSample,
    Clear,
    SetTemplate(Option<AudioTemplate>, f32),
    SetThreshold(f32),
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Live {
    /// „unsupported” | „waiting” | „capturing” | „disabled” | „error”
    pub capture: String,
    pub capture_target: String,
    pub capture_error: String,
    pub frame_w: u32,
    pub frame_h: u32,
    pub fps: f32,
    pub analyze_ms: f32,
    pub visual_score: f32,
    pub prompt_visible: bool,
    /// „unsupported” | „off” | „listening” | „error”
    pub audio: String,
    pub audio_device: String,
    pub audio_score: f32,
    pub key_down: bool,
    /// OCR-ul din Windows: „off” | „ready” | „missing” | „error”.
    pub ocr: String,
    pub ocr_error: String,
    /// Ultimul text de prompt citit (gol dacă nu se vede).
    pub prompt_text: String,
    /// Cât auriu e lângă prompt (0..1).
    pub gold: f32,
}

/// Un candidat văzut de detectorul automat (pentru diagnostic), în pixelii zonei de căutare.
#[derive(Clone, Debug, Default, Serialize)]
pub struct DiagCand {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// A fost citit cu OCR.
    pub ocr: bool,
    pub text: String,
    /// Textul citit conține cuvântul promptului.
    pub matched: bool,
    pub gold: f32,
    /// Găsit de plasa de siguranță (OCR pe zona centrală), nu de căutarea rapidă.
    pub fallback: bool,
}

impl DiagCand {
    pub fn from_box(b: prompt::BoxI) -> Self {
        Self { x: b.x, y: b.y, w: b.w, h: b.h, ..Default::default() }
    }
}

/// Ce a văzut detectorul automat la ultimul cadru (pentru pagina de diagnostic și F5).
#[derive(Clone, Debug, Default, Serialize)]
pub struct AutoDiag {
    pub region_w: usize,
    pub region_h: usize,
    pub cands: Vec<DiagCand>,
    pub ocr_lang: String,
    pub find_ms: f32,
    pub ocr_ms: f32,
    pub frames: u64,
    pub ocr_runs: u64,
    pub fallback_runs: u64,
    /// Ce a citit ultima dată plasa de siguranță.
    pub fallback_text: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChestPayload {
    pub total: i64,
    pub session: i64,
    pub today: i64,
    pub delta: i32,
    pub source: Source,
    pub ts: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct CalibPayload {
    pub kind: &'static str,
    pub ok: bool,
    pub message: String,
    pub samples: usize,
    pub ready: bool,
    pub score: f32,
}

pub struct Thumb {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

pub struct Engine {
    pub app: AppHandle,
    pub store: Mutex<Store>,
    pub fusion: Mutex<Fusion>,
    pub live: Mutex<Live>,
    pub visual: RwLock<VisualCfg>,
    pub visual_sample_req: AtomicBool,
    pub thumb_req: AtomicBool,
    pub thumb: Mutex<Option<Thumb>>,
    pub last_roi: Mutex<Option<Gray>>,
    pub audio_cmds: Mutex<VecDeque<AudioCmd>>,
    pub audio_enabled: AtomicBool,
    pub restart_capture: AtomicBool,
    pub key_down: AtomicBool,
    pub last_key_ms: AtomicU64,
    pub interact_vk: AtomicU32,
    pub gamepad: AtomicBool,
    pub audio_score_bits: AtomicU32,
    pub audio_samples: AtomicU32,
    pub shutdown: AtomicBool,
    /// Overlay-ul e în modul „mută-mă” (primește mouse-ul).
    pub overlay_editing: AtomicBool,
    /// Ultimul cadru pentru detectorul automat (număr de ordine, imagine).
    pub auto_frame: Mutex<Option<(u64, Arc<Rgb>)>>,
    pub auto_cv: Condvar,
    /// Ultimul decupaj dat la OCR și ce a citit (pentru previzualizare).
    pub last_ocr: Mutex<Option<(OcrImage, String)>>,
    pub diag: Mutex<AutoDiag>,
    /// Ultimul cadru analizat de detectorul automat (zona de căutare).
    pub diag_frame: Mutex<Option<Arc<Rgb>>>,
    start: Instant,
}

/// Prag vizual automat din scorul de calibrare.
pub fn auto_visual_threshold(calib_score: f32) -> f32 {
    (calib_score * 0.72).clamp(0.45, 0.85)
}

impl Engine {
    pub fn new(app: AppHandle, store: Store) -> Arc<Self> {
        let s = &store.settings;
        let tpl = store.calib.visual.clone().map(Arc::new);
        let threshold = s
            .visual_threshold
            .unwrap_or_else(|| tpl.as_ref().map_or(0.6, |t| auto_visual_threshold(t.calib_score)));
        let visual = VisualCfg {
            enabled: s.visual_enabled,
            tpl,
            threshold,
            roi: s.roi,
            fps: s.fps.clamp(5, 30),
            require_key: s.require_key,
            method: s.visual_method,
            chest_only: s.chest_only,
            words: words_of(s),
        };
        let audio_tpl = store.calib.audio.clone();
        let audio_thr = s
            .audio_threshold
            .unwrap_or_else(|| audio_tpl.as_ref().map_or(0.6, |t| (t.calib_score * 0.8).clamp(0.45, 0.85)));
        let mut cmds = VecDeque::new();
        cmds.push_back(AudioCmd::SetTemplate(audio_tpl, audio_thr));
        Arc::new(Self {
            app,
            fusion: Mutex::new(Fusion::new(s.mode)),
            live: Mutex::new(Live { capture: "waiting".into(), audio: "off".into(), ocr: "off".into(), ..Default::default() }),
            visual: RwLock::new(visual),
            visual_sample_req: AtomicBool::new(false),
            thumb_req: AtomicBool::new(false),
            thumb: Mutex::new(None),
            last_roi: Mutex::new(None),
            audio_cmds: Mutex::new(cmds),
            audio_enabled: AtomicBool::new(s.audio_enabled),
            restart_capture: AtomicBool::new(false),
            key_down: AtomicBool::new(false),
            last_key_ms: AtomicU64::new(0),
            interact_vk: AtomicU32::new(s.interact_vk),
            gamepad: AtomicBool::new(s.gamepad),
            audio_score_bits: AtomicU32::new(0),
            audio_samples: AtomicU32::new(0),
            shutdown: AtomicBool::new(false),
            overlay_editing: AtomicBool::new(false),
            auto_frame: Mutex::new(None),
            auto_cv: Condvar::new(),
            last_ocr: Mutex::new(None),
            diag: Mutex::new(AutoDiag::default()),
            diag_frame: Mutex::new(None),
            start: Instant::now(),
            store: Mutex::new(store),
        })
    }

    /// Milisecunde monotone de la pornire.
    pub fn mono_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    /// Tasta de interacțiune e apăsată acum sau a fost în ultimele 700 ms.
    pub fn key_recent(&self) -> bool {
        self.key_down.load(Ordering::Relaxed)
            || self.mono_ms().saturating_sub(self.last_key_ms.load(Ordering::Relaxed)) < 700
    }

    /// Ultima dată (ms monotone) când tasta de interacțiune era apăsată.
    pub fn last_key(&self) -> Option<u64> {
        if self.key_down.load(Ordering::Relaxed) {
            return Some(self.mono_ms());
        }
        match self.last_key_ms.load(Ordering::Relaxed) {
            0 => None,
            t => Some(t),
        }
    }

    /// Detectorul automat trebuie să citească des: ai apăsat tasta de curând sau se vede un prompt.
    pub fn auto_active(&self) -> bool {
        let now = self.mono_ms();
        self.last_key().is_some_and(|k| now.saturating_sub(k) <= prompt::KEY_WINDOW_MS) || OCR_TRACKER.lock().visible()
    }

    /// Rezultatul detectorului automat pentru un cadru: s-a văzut promptul („SEARCH”) și cât auriu are lângă el.
    pub fn on_prompt_obs(&self, found: bool, text: &str, gold: f32) {
        let cfg = self.visual.read().clone();
        let now = self.mono_ms();
        let (opened, visible, gold_max) = {
            let mut t = OCR_TRACKER.lock();
            let opened = t.update(found, gold, now, self.last_key(), cfg.require_key, cfg.chest_only);
            (opened, t.visible(), t.gold_max())
        };
        let chest = |g: f32| !cfg.chest_only || g >= prompt::GOLD_MIN;
        {
            let mut l = self.live.lock();
            l.visual_score = match (found, chest(gold)) {
                (true, true) => 1.0,
                (true, false) => 0.5,
                _ => 0.0,
            };
            l.prompt_visible = visible && chest(gold_max);
            l.prompt_text = if found { text.to_string() } else { String::new() };
            l.gold = gold;
        }
        if opened {
            self.signal(Source::Visual);
        }
    }

    /// Starea completă a detecției (pentru diagnostic): setări, stare live, ce s-a văzut.
    pub fn diag_json(&self) -> serde_json::Value {
        let live = self.live.lock().clone();
        let diag = self.diag.lock().clone();
        let s = self.store.lock().settings.clone();
        let now = self.mono_ms();
        serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "live": live,
            "diag": diag,
            "key_ms_ago": self.last_key().map(|k| now.saturating_sub(k)),
            "settings": {
                "visual_method": s.visual_method,
                "visual_enabled": s.visual_enabled,
                "chest_only": s.chest_only,
                "search_words": s.search_words,
                "require_key": s.require_key,
                "interact_label": s.interact_label,
                "interact_vk": s.interact_vk,
                "capture_source": s.capture_source,
                "mode": s.mode,
                "fps": s.fps,
            },
        })
    }

    /// Salvează diagnosticul (imaginea zonei de căutare + starea) în folderul aplicației.
    /// Întoarce folderul.
    pub fn save_diagnostic(&self) -> Result<std::path::PathBuf, String> {
        let dir = self.store.lock().dir().join("diagnostic");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
        let frame = self.diag_frame.lock().clone();
        if let Some(img) = &frame {
            let path = dir.join(format!("diagnostic_{stamp}.png"));
            let file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
            let mut enc = png::Encoder::new(std::io::BufWriter::new(file), img.w as u32, img.h as u32);
            enc.set_color(png::ColorType::Rgb);
            enc.set_depth(png::BitDepth::Eight);
            let mut wr = enc.write_header().map_err(|e| e.to_string())?;
            wr.write_image_data(&img.px).map_err(|e| e.to_string())?;
        }
        let mut json = self.diag_json();
        json["has_image"] = serde_json::json!(frame.is_some());
        let text = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?;
        std::fs::write(dir.join(format!("diagnostic_{stamp}.json")), text).map_err(|e| e.to_string())?;
        Ok(dir)
    }

    /// Previzualizarea zonei de căutare (RGBA, micșorată la cel mult `max_w`), pentru diagnostic.
    pub fn diag_image(&self, max_w: usize) -> Vec<u8> {
        let frame = self.diag_frame.lock().clone();
        let mut out = Vec::new();
        let Some(img) = frame else {
            out.extend_from_slice(&[0u8; 8]);
            return out;
        };
        let ow = img.w.min(max_w).max(1);
        let oh = (img.h * ow / img.w.max(1)).max(1);
        out.extend_from_slice(&(ow as u32).to_le_bytes());
        out.extend_from_slice(&(oh as u32).to_le_bytes());
        out.reserve(ow * oh * 4);
        for oy in 0..oh {
            let sy = oy * img.h / oh;
            for ox in 0..ow {
                let sx = ox * img.w / ow;
                let i = (sy * img.w + sx) * 3;
                out.extend_from_slice(&[img.px[i], img.px[i + 1], img.px[i + 2], 255]);
            }
        }
        out
    }

    /// Un semnal de la un detector.
    pub fn signal(&self, src: Source) {
        let now = self.mono_ms();
        let decided = self.fusion.lock().signal(src, now);
        if let Some(source) = decided {
            self.count(1, source);
        }
    }

    pub fn count(&self, delta: i32, source: Source) -> bool {
        let payload = {
            let mut st = self.store.lock();
            if !st.add(delta, source) {
                return false;
            }
            ChestPayload {
                total: st.data.total,
                session: st.session.count,
                today: st.today(),
                delta,
                source,
                ts: now_ms(),
            }
        };
        let _ = self.app.emit("chest", &payload);
        crate::tray::update_tooltip(&self.app, payload.session, payload.today);
        true
    }

    pub fn emit_calib(&self, p: CalibPayload) {
        let _ = self.app.emit("calib", &p);
    }

    /// Aplică setările noi (după ce au fost salvate în store).
    pub fn apply_settings(&self, s: &Settings, prev: &Settings) {
        self.fusion.lock().mode = s.mode;
        {
            let mut v = self.visual.write();
            v.enabled = s.visual_enabled;
            v.roi = s.roi.sanitized();
            v.fps = s.fps.clamp(5, 30);
            v.require_key = s.require_key;
            v.chest_only = s.chest_only;
            v.words = words_of(s);
            if v.method != s.visual_method {
                v.method = s.visual_method;
                *TRACKER.lock() = None;
                *OCR_TRACKER.lock() = OcrTracker::default();
            }
            v.threshold = s
                .visual_threshold
                .unwrap_or_else(|| v.tpl.as_ref().map_or(0.6, |t| auto_visual_threshold(t.calib_score)));
        }
        if let Some(t) = s.audio_threshold {
            self.audio_cmds.lock().push_back(AudioCmd::SetThreshold(t));
        } else if prev.audio_threshold.is_some() {
            let tpl = self.store.lock().calib.audio.clone();
            let thr = tpl.as_ref().map_or(0.6, |t| (t.calib_score * 0.8).clamp(0.45, 0.85));
            self.audio_cmds.lock().push_back(AudioCmd::SetThreshold(thr));
        }
        self.audio_enabled.store(s.audio_enabled, Ordering::Relaxed);
        self.interact_vk.store(s.interact_vk, Ordering::Relaxed);
        self.gamepad.store(s.gamepad, Ordering::Relaxed);
        if s.capture_source != prev.capture_source || s.monitor_index != prev.monitor_index || s.visual_enabled != prev.visual_enabled {
            self.restart_capture.store(true, Ordering::Relaxed);
        }
    }

    /// Apelat de firul de captură cu imaginea zonei (deja micșorată).
    pub fn on_roi_frame(&self, img: Gray, frame_w: u32, frame_h: u32) {
        let cfg = self.visual.read().clone();
        let now = self.mono_ms();

        if self.visual_sample_req.swap(false, Ordering::Relaxed) {
            self.add_visual_sample(img.clone(), frame_w, frame_h, cfg.roi);
        }

        let (score, visible, opened) = match &cfg.tpl {
            Some(tpl) if tpl.w == img.w && tpl.h == img.h => {
                let (score, _) = visual::match_score(tpl, &img);
                let mut tracker = TRACKER.lock();
                let tr = tracker.get_or_insert_with(|| visual::PromptTracker::new(cfg.threshold));
                if (tr.on_threshold - cfg.threshold).abs() > 1e-4 {
                    tr.set_threshold(cfg.threshold);
                }
                let opened = tr.update(score, now, self.key_recent(), cfg.require_key);
                (score, tr.visible(), opened)
            }
            _ => (0.0, false, false),
        };
        {
            let mut l = self.live.lock();
            l.visual_score = score;
            l.prompt_visible = visible;
            l.frame_w = frame_w;
            l.frame_h = frame_h;
        }
        *self.last_roi.lock() = Some(img);
        if opened {
            self.signal(Source::Visual);
        }
    }

    fn add_visual_sample(&self, img: Gray, fw: u32, fh: u32, roi: Roi) {
        let mut st = self.store.lock();
        let c = &mut st.calib;
        let changed = c.visual_frame != Some((fw, fh))
            || c.visual_roi.is_none_or(|r| r != roi)
            || c.visual_samples.first().is_some_and(|s| s.w != img.w || s.h != img.h);
        let mut note = String::new();
        if changed && !c.visual_samples.is_empty() {
            c.visual_samples.clear();
            note = "Rezoluția sau zona s-a schimbat, am început probele de la zero. ".into();
        }
        c.visual_frame = Some((fw, fh));
        c.visual_roi = Some(roi);
        c.visual_samples.push(img);
        if c.visual_samples.len() > 8 {
            c.visual_samples.remove(0);
        }
        let n = c.visual_samples.len();
        let payload = if n < 3 {
            CalibPayload {
                kind: "visual",
                ok: true,
                message: format!("{note}Proba {n} salvată. Mai fă {} la alt cufăr, în alt loc.", 3 - n),
                samples: n,
                ready: c.visual.is_some(),
                score: 0.0,
            }
        } else {
            match visual::build_template(&c.visual_samples) {
                Ok(tpl) => {
                    let score = tpl.calib_score;
                    let tpl = Arc::new(tpl);
                    c.visual = Some((*tpl).clone());
                    let manual = st.settings.visual_threshold;
                    {
                        let mut v = self.visual.write();
                        v.tpl = Some(tpl);
                        v.threshold = manual.unwrap_or_else(|| auto_visual_threshold(score));
                    }
                    *TRACKER.lock() = None;
                    CalibPayload {
                        kind: "visual",
                        ok: true,
                        message: format!("{note}Detecția vizuală e gata ({n} probe, potrivire {:.0}%).", score * 100.0),
                        samples: n,
                        ready: true,
                        score,
                    }
                }
                Err(e) => {
                    // O probă proastă: o scoatem, ca utilizatorul să poată reface doar una.
                    if n >= 5 {
                        st.calib.visual_samples.remove(n - 1);
                    }
                    CalibPayload {
                        kind: "visual",
                        ok: false,
                        message: format!("{note}{e}"),
                        samples: st.calib.visual_samples.len(),
                        ready: st.calib.visual.is_some(),
                        score: 0.0,
                    }
                }
            }
        };
        st.save_calib();
        drop(st);
        self.emit_calib(payload);
    }

    pub fn clear_visual(&self) {
        let mut st = self.store.lock();
        st.calib.visual = None;
        st.calib.visual_samples.clear();
        st.calib.visual_frame = None;
        st.calib.visual_roi = None;
        st.save_calib();
        drop(st);
        self.visual.write().tpl = None;
        *TRACKER.lock() = None;
    }

    /// Imaginea curentă a zonei + șablonul + masca, pentru previzualizare (format binar).
    /// În modul automat: ultimul decupaj citit cu OCR.
    pub fn visual_preview(&self) -> Vec<u8> {
        if self.visual.read().auto() {
            let last = self.last_ocr.lock();
            let mut out = Vec::new();
            match &*last {
                Some((img, _)) => {
                    out.extend_from_slice(&(img.w as u32).to_le_bytes());
                    out.extend_from_slice(&(img.h as u32).to_le_bytes());
                    out.push(0);
                    out.extend_from_slice(&img.px);
                }
                None => {
                    out.extend_from_slice(&[0u8; 8]);
                    out.push(0);
                }
            }
            return out;
        }
        let cur = self.last_roi.lock().clone();
        let tpl = self.visual.read().tpl.clone();
        let (w, h) = match (&cur, &tpl) {
            (Some(c), _) => (c.w, c.h),
            (None, Some(t)) => (t.w, t.h),
            _ => (0, 0),
        };
        let mut out = Vec::with_capacity(9 + w * h * 3);
        out.extend_from_slice(&(w as u32).to_le_bytes());
        out.extend_from_slice(&(h as u32).to_le_bytes());
        let tpl = tpl.filter(|t| t.w == w && t.h == h);
        out.push(u8::from(tpl.is_some()));
        match &cur {
            Some(c) => out.extend(c.to_u8()),
            None => out.extend(std::iter::repeat_n(0u8, w * h)),
        }
        if let Some(t) = &tpl {
            out.extend(t.mean.to_u8());
            out.extend(t.mask.iter().map(|&m| m * 255));
        }
        out
    }
}

/// Starea promptului (un singur fir de captură o folosește).
static TRACKER: Mutex<Option<visual::PromptTracker>> = Mutex::new(None);
/// Starea promptului citit cu OCR (detectorul automat).
static OCR_TRACKER: Mutex<OcrTracker> = Mutex::new(OcrTracker::new());
