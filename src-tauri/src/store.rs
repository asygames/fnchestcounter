//! Salvarea datelor: numărătoarea, istoricul pe zile, sesiunile și setările.
//! Scrierea e atomică (fișier temporar + redenumire), ca să nu se strice la o oprire bruscă.

use crate::detect::{audio::AudioTemplate, fusion::Mode, fusion::Source, visual::Roi, visual::Template};
use chrono::{Local, TimeZone};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

const MAX_EVENTS: usize = 2000;
const MAX_SESSIONS: usize = 200;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChestEvent {
    /// Milisecunde Unix.
    pub ts: i64,
    pub source: Source,
    /// +1 sau −1 (corecție manuală).
    pub delta: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: i64,
    pub start: i64,
    pub end: i64,
    pub count: i64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Data {
    pub version: u32,
    pub total: i64,
    /// „AAAA-LL-ZZ” → cufere în ziua aceea.
    pub days: BTreeMap<String, i64>,
    pub events: VecDeque<ChestEvent>,
    pub sessions: VecDeque<Session>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub mode: Mode,
    pub roi: Roi,
    /// Cadre analizate pe secundă (5–30).
    pub fps: u32,
    /// Pragul vizual (0,3–0,95). `None` = automat după calibrare.
    pub visual_threshold: Option<f32>,
    pub audio_threshold: Option<f32>,
    pub audio_enabled: bool,
    pub visual_enabled: bool,
    /// Codul de tastă virtuală Windows pentru interacțiune (implicit E = 0x45).
    pub interact_vk: u32,
    pub interact_label: String,
    pub gamepad: bool,
    /// Numără vizual doar dacă a fost apăsată tasta de interacțiune.
    pub require_key: bool,
    pub capture_source: CaptureSource,
    pub monitor_index: usize,
    pub overlay: bool,
    pub overlay_x: Option<i32>,
    pub overlay_y: Option<i32>,
    pub overlay_scale: f32,
    pub close_to_tray: bool,
    pub sounds: bool,
    pub hotkey_add: String,
    pub hotkey_sub: String,
    pub hotkey_visual_sample: String,
    pub hotkey_audio_sample: String,
    pub reduce_motion: bool,
    /// Cum găsim promptul: automat (OCR, fără calibrare) sau cu șablonul din probe.
    pub visual_method: VisualMethod,
    /// Numără doar cuferele (aurii), nu și lăzile de muniție (au tot „SEARCH”).
    pub chest_only: bool,
    /// Cuvintele promptului, separate prin virgulă (pentru jocul în altă limbă).
    pub search_words: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum VisualMethod {
    /// Citește textul promptului („SEARCH”) oriunde lângă centrul ecranului. Recomandat.
    #[default]
    Auto,
    /// Șablonul învățat din probe, într-o zonă fixă (metoda veche).
    Template,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CaptureSource {
    /// Fereastra Fortnite (recomandat).
    Window,
    /// Un monitor întreg.
    Monitor,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mode: Mode::Either,
            roi: Roi::default(),
            fps: 15,
            visual_threshold: None,
            audio_threshold: None,
            audio_enabled: true,
            visual_enabled: true,
            interact_vk: 0x45,
            interact_label: "E".into(),
            gamepad: true,
            require_key: true,
            capture_source: CaptureSource::Window,
            monitor_index: 0,
            overlay: true,
            overlay_x: None,
            overlay_y: None,
            overlay_scale: 1.0,
            close_to_tray: true,
            sounds: true,
            hotkey_add: "F9".into(),
            hotkey_sub: "F10".into(),
            hotkey_visual_sample: "F8".into(),
            hotkey_audio_sample: "F7".into(),
            reduce_motion: false,
            visual_method: VisualMethod::Auto,
            chest_only: true,
            search_words: "SEARCH".into(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Calibration {
    pub visual: Option<Template>,
    /// Probele vizuale strânse până acum (se păstrează între reporniri).
    pub visual_samples: Vec<crate::detect::visual::Gray>,
    /// Pe ce rezoluție a fost făcută calibrarea vizuală.
    pub visual_frame: Option<(u32, u32)>,
    pub visual_roi: Option<Roi>,
    pub audio: Option<AudioTemplate>,
}

pub struct Store {
    dir: PathBuf,
    pub data: Data,
    pub settings: Settings,
    pub calib: Calibration,
    pub session: Session,
}

fn read_json<T: DeserializeOwned + Default>(path: &Path) -> T {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            log::warn!("{}: {e}; păstrez o copie și pornesc de la zero", path.display());
            let _ = fs::copy(path, path.with_extension("corupt.json"));
            T::default()
        }),
        Err(_) => T::default(),
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    let bytes = serde_json::to_vec(value).map_err(std::io::Error::other)?;
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

pub fn today_key() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

fn day_key(ts_ms: i64) -> String {
    Local.timestamp_millis_opt(ts_ms).single().unwrap_or_else(Local::now).format("%Y-%m-%d").to_string()
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

impl Store {
    pub fn open(dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&dir);
        let mut data: Data = read_json(&dir.join("data.json"));
        data.version = 1;
        let settings: Settings = read_json(&dir.join("settings.json"));
        let calib: Calibration = read_json(&dir.join("calibration.json"));
        let now = now_ms();
        let session = Session { id: now, start: now, end: now, count: 0 };
        Self { dir, data, settings, calib, session }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Adaugă (+1) sau scade (−1) un cufăr. Totalul nu scade sub zero.
    pub fn add(&mut self, delta: i32, source: Source) -> bool {
        let today = today_key();
        let day = self.data.days.entry(today).or_insert(0);
        if delta < 0 && (self.session.count <= 0 || *day <= 0 || self.data.total <= 0) {
            return false;
        }
        *day += delta as i64;
        self.data.total += delta as i64;
        self.session.count += delta as i64;
        let ts = now_ms();
        self.session.end = ts;
        self.data.events.push_back(ChestEvent { ts, source, delta });
        while self.data.events.len() > MAX_EVENTS {
            self.data.events.pop_front();
        }
        self.upsert_session();
        self.save_data();
        true
    }

    fn upsert_session(&mut self) {
        if let Some(last) = self.data.sessions.back_mut() {
            if last.id == self.session.id {
                *last = self.session.clone();
                return;
            }
        }
        if self.session.count > 0 {
            self.data.sessions.push_back(self.session.clone());
            while self.data.sessions.len() > MAX_SESSIONS {
                self.data.sessions.pop_front();
            }
        }
    }

    /// Începe o sesiune nouă (contorul „Sesiune” revine la 0; totalul rămâne).
    pub fn new_session(&mut self) {
        let now = now_ms();
        self.session = Session { id: now, start: now, end: now, count: 0 };
    }

    pub fn today(&self) -> i64 {
        *self.data.days.get(&today_key()).unwrap_or(&0)
    }

    /// Șterge tot istoricul (cu confirmare în UI).
    pub fn reset_all(&mut self) {
        self.data = Data { version: 1, ..Default::default() };
        self.new_session();
        self.save_data();
    }

    pub fn save_data(&self) {
        if let Err(e) = write_json(&self.dir.join("data.json"), &self.data) {
            log::error!("nu pot salva datele: {e}");
        }
    }

    pub fn save_settings(&self) {
        if let Err(e) = write_json(&self.dir.join("settings.json"), &self.settings) {
            log::error!("nu pot salva setările: {e}");
        }
    }

    pub fn save_calib(&self) {
        if let Err(e) = write_json(&self.dir.join("calibration.json"), &self.calib) {
            log::error!("nu pot salva calibrarea: {e}");
        }
    }

    /// Recalculează zilele din evenimente (folosit doar la teste/verificări).
    #[allow(dead_code)]
    pub fn days_from_events(&self) -> BTreeMap<String, i64> {
        let mut m = BTreeMap::new();
        for e in &self.data.events {
            *m.entry(day_key(e.ts)).or_insert(0) += e.delta as i64;
        }
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("fncc-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn persists_and_reloads() {
        let dir = tmpdir("persist");
        {
            let mut s = Store::open(dir.clone());
            assert!(s.add(1, Source::Visual));
            assert!(s.add(1, Source::Audio));
            assert!(s.add(-1, Source::Manual));
            assert_eq!(s.session.count, 1);
            s.settings.fps = 20;
            s.save_settings();
        }
        let s = Store::open(dir.clone());
        assert_eq!(s.data.total, 1);
        assert_eq!(s.today(), 1);
        assert_eq!(s.data.events.len(), 3);
        assert_eq!(s.data.sessions.len(), 1);
        assert_eq!(s.settings.fps, 20);
        assert_eq!(s.session.count, 0, "sesiune nouă la pornire");
        assert_eq!(s.days_from_events(), s.data.days);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn never_goes_negative_and_survives_corrupt_file() {
        let dir = tmpdir("neg");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("data.json"), b"{nu e json").unwrap();
        let mut s = Store::open(dir.clone());
        assert_eq!(s.data.total, 0);
        assert!(dir.join("data.corupt.json").exists());
        assert!(!s.add(-1, Source::Manual));
        assert_eq!(s.data.total, 0);
        let _ = fs::remove_dir_all(dir);
    }
}
