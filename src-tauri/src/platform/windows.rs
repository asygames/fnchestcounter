//! Windows: captura (Windows Graphics Capture), sunetul (WASAPI loopback) și tasta de interacțiune.
//!
//! Toate sunt doar CITIRI, exact ca OBS / Discord: nu injectăm nimic în joc,
//! nu citim memoria lui și nu trimitem taste.

use crate::detect::audio::{AudioDetector, FeatureExtractor};
use crate::detect::fusion::Source;
use crate::detect::prompt::{self, OcrImage};
use crate::detect::visual::{downscale_gray, WORK_W};
use crate::engine::{AudioCmd, CalibPayload, Engine, Thumb};
use crate::store::CaptureSource;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::{GraphicsCaptureApi, InternalCaptureControl};
use windows_capture::monitor::Monitor;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings, MinimumUpdateIntervalSettings,
    SecondaryWindowSettings, Settings,
};
use windows_capture::window::Window;

const FORTNITE_EXE: &str = "fortniteclient-win64-shipping.exe";

pub fn start(engine: Arc<Engine>) {
    let e = engine.clone();
    thread::Builder::new().name("capture".into()).spawn(move || capture_loop(e)).ok();
    let e = engine.clone();
    thread::Builder::new().name("ocr".into()).spawn(move || ocr_loop(e)).ok();
    let e = engine.clone();
    thread::Builder::new().name("audio".into()).spawn(move || audio_loop(e)).ok();
    let e = engine;
    thread::Builder::new().name("keys".into()).spawn(move || key_loop(e)).ok();
}

// ───────────────────────────── Captură ─────────────────────────────

fn find_fortnite() -> Option<Window> {
    let windows = Window::enumerate().ok()?;
    windows
        .into_iter()
        .filter(|w| w.is_valid())
        .find(|w| w.process_name().map(|n| n.to_lowercase() == FORTNITE_EXE).unwrap_or(false))
}

struct Capture {
    engine: Arc<Engine>,
    last: Option<Instant>,
    frames: u32,
    fps_t0: Instant,
}

impl GraphicsCaptureApiHandler for Capture {
    type Flags = Arc<Engine>;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self { engine: ctx.flags, last: None, frames: 0, fps_t0: Instant::now() })
    }

    fn on_frame_arrived(&mut self, frame: &mut Frame, control: InternalCaptureControl) -> Result<(), Self::Error> {
        let e = &self.engine;
        if e.shutdown.load(Ordering::Relaxed) || e.restart_capture.load(Ordering::Relaxed) {
            control.stop();
            return Ok(());
        }
        let cfg = e.visual.read().clone();
        // Detectorul automat copiază o zonă mare: 10 cadre/s îi ajung și nu încarcă placa video.
        let fps = if cfg.auto() { cfg.fps.min(10) } else { cfg.fps };
        let interval = Duration::from_millis(1000 / fps.max(1) as u64);
        let now = Instant::now();
        let thumb_wanted = e.thumb_req.load(Ordering::Relaxed);
        if !thumb_wanted && self.last.is_some_and(|t| now.duration_since(t) < interval) {
            // Cadru sărit: nu copiem nimic din GPU.
            return Ok(());
        }
        self.last = Some(now);
        let t0 = Instant::now();
        let (fw, fh) = (frame.width(), frame.height());
        if fw < 64 || fh < 64 {
            return Ok(());
        }

        if thumb_wanted && e.thumb_req.swap(false, Ordering::Relaxed) {
            if let Ok(mut buf) = frame.buffer() {
                let (w, h, pitch) = (buf.width() as usize, buf.height() as usize, buf.row_pitch() as usize);
                let raw = buf.as_raw_buffer();
                *e.thumb.lock() = Some(make_thumb(raw, w, h, pitch, 480));
            }
        }

        if cfg.auto() {
            // Promptul se mută odată cu cufărul: luăm toată zona centrală, color,
            // micșorată cel mult la 1440p. Analiza (OCR) o face firul „ocr”.
            let (x0, y0, x1, y1) = prompt::SEARCH_REGION.pixels(fw, fh);
            let s = (prompt::MAX_FRAME_H as f32 / fh as f32).min(1.0);
            let img = {
                let mut buf = frame.buffer_crop(x0, y0, x1, y1)?;
                let (w, h, pitch) = (buf.width() as usize, buf.height() as usize, buf.row_pitch() as usize);
                let raw = buf.as_raw_buffer();
                let (ow, oh) = (((w as f32 * s) as usize).max(1), ((h as f32 * s) as usize).max(1));
                prompt::downscale_rgb(raw, w, h, pitch, true, ow, oh)
            };
            {
                let mut slot = e.auto_frame.lock();
                let seq = slot.as_ref().map_or(0, |f| f.0) + 1;
                *slot = Some((seq, Arc::new(img)));
            }
            e.auto_cv.notify_one();
            {
                let mut l = e.live.lock();
                l.frame_w = fw;
                l.frame_h = fh;
            }
            self.tick_fps();
            return Ok(());
        }

        // Copiem din GPU doar zona de interes, nu tot cadrul.
        let (x0, y0, x1, y1) = cfg.roi.pixels(fw, fh);
        let work_h = cfg.roi.work_h(fw, fh);
        let img = {
            let mut buf = frame.buffer_crop(x0, y0, x1, y1)?;
            let (w, h, pitch) = (buf.width() as usize, buf.height() as usize, buf.row_pitch() as usize);
            let raw = buf.as_raw_buffer();
            downscale_gray(raw, w, h, pitch, true, WORK_W, work_h)
        };
        e.on_roi_frame(img, fw, fh);

        let analyze_ms = t0.elapsed().as_secs_f32() * 1000.0;
        {
            let mut l = e.live.lock();
            l.analyze_ms = l.analyze_ms * 0.9 + analyze_ms * 0.1;
        }
        self.tick_fps();
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl Capture {
    fn tick_fps(&mut self) {
        self.frames += 1;
        let el = self.fps_t0.elapsed();
        if el >= Duration::from_secs(1) {
            self.engine.live.lock().fps = self.frames as f32 / el.as_secs_f32();
            self.frames = 0;
            self.fps_t0 = Instant::now();
        }
    }
}

// ───────────────────────────── OCR (detectorul automat) ─────────────────────────────

/// OCR-ul inclus în Windows 10/11 (Windows.Media.Ocr). Nu trebuie instalat nimic în plus,
/// doar o limbă cu alfabet latin (engleza vine aproape mereu cu Windows).
struct Ocr {
    engine: windows::Media::Ocr::OcrEngine,
    lang: String,
}

impl Ocr {
    fn new() -> Result<Self, String> {
        use windows::core::HSTRING;
        use windows::Globalization::Language;
        use windows::Media::Ocr::OcrEngine;

        let make = |engine: OcrEngine| {
            let lang = engine.RecognizerLanguage().and_then(|l| l.LanguageTag()).map(|t| t.to_string()).unwrap_or_default();
            Self { engine, lang }
        };
        // Promptul e în engleză („SEARCH”): preferăm recunoașterea pentru engleză.
        for tag in ["en-US", "en-GB", "en"] {
            let Ok(lang) = Language::CreateLanguage(&HSTRING::from(tag)) else { continue };
            if OcrEngine::IsLanguageSupported(&lang).unwrap_or(false) {
                if let Ok(engine) = OcrEngine::TryCreateFromLanguage(&lang) {
                    return Ok(make(engine));
                }
            }
        }
        if let Ok(engine) = OcrEngine::TryCreateFromUserProfileLanguages() {
            return Ok(make(engine));
        }
        if let Ok(langs) = OcrEngine::AvailableRecognizerLanguages() {
            for i in 0..langs.Size().unwrap_or(0) {
                if let Ok(engine) = langs.GetAt(i).and_then(|l| OcrEngine::TryCreateFromLanguage(&l)) {
                    return Ok(make(engine));
                }
            }
        }
        Err("Windows nu are recunoaștere de text (OCR) pentru nicio limbă. Adaugă limba English (United States) din Setări Windows → Oră și limbă → Limbă și regiune, apoi repornește aplicația.".into())
    }

    /// Cuvintele citite, cu dreptunghiul fiecăruia (în pixelii imaginii date).
    fn read_words(&self, img: &OcrImage) -> windows::core::Result<Vec<(String, windows::Foundation::Rect)>> {
        let res = self.recognize(img)?;
        let mut out = Vec::new();
        let lines = res.Lines()?;
        for i in 0..lines.Size()? {
            let words = lines.GetAt(i)?.Words()?;
            for j in 0..words.Size()? {
                let w = words.GetAt(j)?;
                out.push((w.Text()?.to_string(), w.BoundingRect()?));
            }
        }
        Ok(out)
    }

    #[cfg(test)]
    fn read(&self, img: &OcrImage) -> windows::core::Result<String> {
        Ok(self.recognize(img)?.Text()?.to_string())
    }

    fn recognize(&self, img: &OcrImage) -> windows::core::Result<windows::Media::Ocr::OcrResult> {
        use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
        use windows::Security::Cryptography::CryptographicBuffer;

        let mut bgra = Vec::with_capacity(img.w * img.h * 4);
        for &v in &img.px {
            bgra.extend_from_slice(&[v, v, v, 255]);
        }
        let buf = CryptographicBuffer::CreateFromByteArray(&bgra)?;
        let bmp = SoftwareBitmap::CreateCopyFromBuffer(&buf, BitmapPixelFormat::Bgra8, img.w as i32, img.h as i32)?;
        self.engine.RecognizeAsync(&bmp)?.join()
    }
}

fn set_ocr(e: &Engine, state: &str, err: &str) {
    let mut l = e.live.lock();
    l.ocr = state.into();
    l.ocr_error = err.into();
}

fn ocr_loop(e: Arc<Engine>) {
    use crate::engine::DiagCand;
    use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};
    unsafe {
        let _ = RoInitialize(RO_INIT_MULTITHREADED);
    }
    let ocr = match Ocr::new() {
        Ok(o) => {
            log::info!("OCR pornit ({})", o.lang);
            set_ocr(&e, "ready", "");
            e.diag.lock().ocr_lang = o.lang.clone();
            Some(o)
        }
        Err(msg) => {
            log::warn!("{msg}");
            set_ocr(&e, "missing", &msg);
            None
        }
    };
    let mut last_seq = 0u64;
    let mut last_ocr = Instant::now() - Duration::from_secs(10);
    let mut last_fallback = Instant::now() - Duration::from_secs(10);
    let mut ocr_errors = 0u32;
    loop {
        if e.shutdown.load(Ordering::Relaxed) {
            return;
        }
        // Așteptăm un cadru nou de la captură (luăm mereu doar ultimul).
        let img = {
            let mut slot = e.auto_frame.lock();
            if slot.as_ref().is_none_or(|f| f.0 == last_seq) {
                e.auto_cv.wait_for(&mut slot, Duration::from_millis(500));
            }
            match slot.as_ref() {
                Some((seq, img)) if *seq != last_seq => {
                    last_seq = *seq;
                    img.clone()
                }
                _ => continue,
            }
        };
        let cfg = e.visual.read().clone();
        if !cfg.auto() || !cfg.enabled {
            continue;
        }
        let t0 = Instant::now();
        let cands = prompt::find_candidates(&img);
        let find_ms = t0.elapsed().as_secs_f32() * 1000.0;
        let now = e.mono_ms();
        let key_recent = e.last_key().is_some_and(|k| now.saturating_sub(k) <= 1500);
        let mut diag: Vec<DiagCand> = cands.iter().map(|c| DiagCand::from_box(c.b)).collect();
        let mut hit: Option<(String, f32)> = None;
        let mut shown: Option<(OcrImage, String)> = None;
        let mut did_ocr = false;
        let mut fallback_text = String::new();

        let read = |o: &Ocr, img: &OcrImage, errors: &mut u32| -> Vec<(String, windows::Foundation::Rect)> {
            match o.read_words(img) {
                Ok(w) => {
                    *errors = 0;
                    w
                }
                Err(err) => {
                    *errors += 1;
                    if *errors == 20 {
                        log::warn!("OCR: {err}");
                        set_ocr(&e, "error", &format!("OCR-ul din Windows dă erori: {err}"));
                    }
                    Vec::new()
                }
            }
        };

        if let Some(ocr) = ocr.as_ref() {
            // 1) Candidații găsiți rapid (text alb pe panou întunecat). Fără tastă și fără
            //    prompt pe ecran citim rar (3 pe secundă), ca să nu consumăm degeaba.
            if !cands.is_empty() && (e.auto_active() || last_ocr.elapsed() >= Duration::from_millis(330)) {
                last_ocr = Instant::now();
                did_ocr = true;
                for (i, c) in cands.iter().take(4).enumerate() {
                    let crop = prompt::ocr_crop(&img, c.b);
                    let words = read(ocr, &crop, &mut ocr_errors);
                    let text = words.iter().map(|w| w.0.as_str()).collect::<Vec<_>>().join(" ");
                    diag[i].ocr = true;
                    diag[i].text = text.clone();
                    if let Some(word) = prompt::match_words(&text, &cfg.words) {
                        let gold = prompt::gold_fraction(&img, c.b);
                        diag[i].matched = true;
                        diag[i].gold = gold;
                        hit = Some((word, gold));
                        shown = Some((crop, text));
                        break;
                    }
                    if shown.is_none() {
                        shown = Some((crop, text));
                    }
                }
            }
            // 2) Plasa de siguranță: ai apăsat tasta și nu am găsit promptul. Citim direct
            //    o zonă mare din jurul centrului ecranului.
            if hit.is_none() && key_recent && last_fallback.elapsed() >= Duration::from_millis(300) {
                last_fallback = Instant::now();
                did_ocr = true;
                let area = prompt::center_area(&img);
                let (oimg, scale) = prompt::ocr_area(&img, area);
                let words = read(ocr, &oimg, &mut ocr_errors);
                fallback_text = words.iter().map(|w| w.0.as_str()).collect::<Vec<_>>().join(" ");
                for (text, r) in &words {
                    if let Some(word) = prompt::match_words(text, &cfg.words) {
                        let b = prompt::BoxI {
                            x: area.x + (r.X / scale) as i32,
                            y: area.y + (r.Y / scale) as i32,
                            w: ((r.Width / scale) as i32).max(1),
                            h: ((r.Height / scale) as i32).max(1),
                        };
                        let gold = prompt::gold_fraction(&img, b);
                        let mut d = DiagCand::from_box(b);
                        d.ocr = true;
                        d.matched = true;
                        d.fallback = true;
                        d.text = text.clone();
                        d.gold = gold;
                        diag.push(d);
                        hit = Some((word, gold));
                        break;
                    }
                }
                e.diag.lock().fallback_runs += 1;
            }
            if ocr_errors == 0 && e.live.lock().ocr != "ready" {
                set_ocr(&e, "ready", "");
            }
        }
        if let Some(s) = shown {
            *e.last_ocr.lock() = Some(s);
        }
        {
            let mut d = e.diag.lock();
            d.region_w = img.w;
            d.region_h = img.h;
            d.cands = diag;
            d.find_ms = find_ms;
            d.frames += 1;
            if did_ocr {
                d.ocr_runs += 1;
                d.ocr_ms = t0.elapsed().as_secs_f32() * 1000.0 - find_ms;
            }
            if !fallback_text.is_empty() {
                d.fallback_text = fallback_text;
            }
        }
        *e.diag_frame.lock() = Some(img.clone());
        match hit {
            Some((word, gold)) => e.on_prompt_obs(true, &word, gold),
            // Un cadru fără citire nu înseamnă „promptul a dispărut” cât timp ții tasta
            // (plasa de siguranță citește doar o dată la 300 ms).
            None if did_ocr || (cands.is_empty() && !key_recent) => e.on_prompt_obs(false, "", 0.0),
            None => {}
        }
        report_ms(&e, t0);
    }
}

fn report_ms(e: &Engine, t0: Instant) {
    let ms = t0.elapsed().as_secs_f32() * 1000.0;
    let mut l = e.live.lock();
    l.analyze_ms = l.analyze_ms * 0.9 + ms * 0.1;
}

fn make_thumb(raw: &[u8], w: usize, h: usize, pitch: usize, out_w: usize) -> Thumb {
    let out_w = out_w.min(w).max(1);
    let out_h = (h * out_w / w.max(1)).max(1);
    let mut rgba = vec![0u8; out_w * out_h * 4];
    for oy in 0..out_h {
        let sy = oy * h / out_h;
        for ox in 0..out_w {
            let sx = ox * w / out_w;
            let i = sy * pitch + sx * 4;
            let o = (oy * out_w + ox) * 4;
            if i + 3 < raw.len() {
                // BGRA → RGBA
                rgba[o] = raw[i + 2];
                rgba[o + 1] = raw[i + 1];
                rgba[o + 2] = raw[i];
                rgba[o + 3] = 255;
            }
        }
    }
    Thumb { w: out_w as u32, h: out_h as u32, rgba }
}

fn set_capture(e: &Engine, state: &str, target: &str, err: &str) {
    let mut l = e.live.lock();
    l.capture = state.into();
    l.capture_target = target.into();
    l.capture_error = err.into();
    if state != "capturing" {
        l.fps = 0.0;
        l.visual_score = 0.0;
        l.prompt_visible = false;
    }
}

fn capture_loop(e: Arc<Engine>) {
    if !GraphicsCaptureApi::is_supported().unwrap_or(false) {
        set_capture(&e, "error", "", "Windows Graphics Capture nu e disponibil (ai nevoie de Windows 10 1903 sau mai nou).");
        return;
    }
    let no_border = GraphicsCaptureApi::is_border_settings_supported().unwrap_or(false);
    let cursor_off = GraphicsCaptureApi::is_cursor_settings_supported().unwrap_or(false);
    loop {
        if e.shutdown.load(Ordering::Relaxed) {
            return;
        }
        e.restart_capture.store(false, Ordering::Relaxed);
        let (enabled, source, monitor_index) = {
            let st = e.store.lock();
            (st.settings.visual_enabled, st.settings.capture_source, st.settings.monitor_index)
        };
        if !enabled {
            set_capture(&e, "disabled", "", "");
            wait_restart(&e, Duration::from_secs(3600));
            continue;
        }
        let border = if no_border { DrawBorderSettings::WithoutBorder } else { DrawBorderSettings::Default };
        let cursor = if cursor_off { CursorCaptureSettings::WithoutCursor } else { CursorCaptureSettings::Default };
        let result = match source {
            CaptureSource::Window => {
                let Some(win) = find_fortnite() else {
                    set_capture(&e, "waiting", "", "");
                    wait_restart(&e, Duration::from_secs(2));
                    continue;
                };
                let title = win.title().unwrap_or_else(|_| "Fortnite".into());
                set_capture(&e, "capturing", &title, "");
                let settings = Settings::new(
                    win,
                    cursor,
                    border,
                    SecondaryWindowSettings::Default,
                    MinimumUpdateIntervalSettings::Default,
                    DirtyRegionSettings::Default,
                    ColorFormat::Bgra8,
                    e.clone(),
                );
                Capture::start_free_threaded(settings).map_err(|err| err.to_string())
            }
            CaptureSource::Monitor => {
                let mon = Monitor::from_index(monitor_index + 1).or_else(|_| Monitor::primary());
                match mon {
                    Ok(mon) => {
                        let name = mon.name().unwrap_or_else(|_| format!("Monitor {}", monitor_index + 1));
                        set_capture(&e, "capturing", &name, "");
                        let settings = Settings::new(
                            mon,
                            cursor,
                            border,
                            SecondaryWindowSettings::Default,
                            MinimumUpdateIntervalSettings::Default,
                            DirtyRegionSettings::Default,
                            ColorFormat::Bgra8,
                            e.clone(),
                        );
                        Capture::start_free_threaded(settings).map_err(|err| err.to_string())
                    }
                    Err(err) => Err(err.to_string()),
                }
            }
        };
        match result {
            Ok(control) => {
                // Așteptăm până se închide jocul, se schimbă setările sau oprim aplicația.
                while !control.is_finished() {
                    if e.shutdown.load(Ordering::Relaxed) || e.restart_capture.load(Ordering::Relaxed) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(250));
                }
                let _ = control.stop();
                set_capture(&e, "waiting", "", "");
                thread::sleep(Duration::from_millis(500));
            }
            Err(err) => {
                log::warn!("captură eșuată: {err}");
                set_capture(&e, "error", "", &format!("Captura nu a pornit: {err}"));
                wait_restart(&e, Duration::from_secs(3));
            }
        }
    }
}

fn wait_restart(e: &Engine, max: Duration) {
    let t0 = Instant::now();
    while t0.elapsed() < max {
        if e.shutdown.load(Ordering::Relaxed) || e.restart_capture.load(Ordering::Relaxed) {
            return;
        }
        thread::sleep(Duration::from_millis(200));
    }
}

// ───────────────────────────── Sunet ─────────────────────────────

fn set_audio(e: &Engine, state: &str, device: &str) {
    let mut l = e.live.lock();
    l.audio = state.into();
    l.audio_device = device.into();
    if state != "listening" {
        l.audio_score = 0.0;
    }
}

fn audio_loop(e: Arc<Engine>) {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use parking_lot::Mutex;
    use std::sync::atomic::AtomicBool;

    // Detectorul trăiește în afara stream-ului, ca să-și păstreze șablonul la reporniri.
    let detector = Arc::new(Mutex::new(AudioDetector::new(None, 0.6)));
    loop {
        if e.shutdown.load(Ordering::Relaxed) {
            return;
        }
        drain_audio_cmds(&e, &mut detector.lock(), None);
        if !e.audio_enabled.load(Ordering::Relaxed) {
            set_audio(&e, "off", "");
            thread::sleep(Duration::from_millis(500));
            continue;
        }
        let host = cpal::default_host();
        let Some(device) = host.default_output_device() else {
            set_audio(&e, "error", "Nu am găsit o ieșire audio");
            thread::sleep(Duration::from_secs(3));
            continue;
        };
        let dev_name = device.description().map(|d| d.to_string()).unwrap_or_else(|_| "Ieșire audio".into());
        let Ok(cfg) = device.default_output_config() else {
            set_audio(&e, "error", &dev_name);
            thread::sleep(Duration::from_secs(3));
            continue;
        };
        let channels = cfg.channels().max(1) as usize;
        let rate = cfg.sample_rate();
        let failed = Arc::new(AtomicBool::new(false));
        let failed_cb = failed.clone();
        let err_fn = move |err: cpal::Error| {
            log::warn!("stream audio: {err}");
            failed_cb.store(true, Ordering::Relaxed);
        };
        let ec = e.clone();
        let det = detector.clone();
        let mut ex = FeatureExtractor::new(rate);
        let mut mono: Vec<f32> = Vec::with_capacity(4096);
        let mut process = move |data: &mut dyn Iterator<Item = f32>| {
            mono.clear();
            let mut acc = 0f32;
            for (i, s) in data.enumerate() {
                acc += s;
                if (i + 1) % channels == 0 {
                    mono.push(acc / channels as f32);
                    acc = 0.0;
                }
            }
            let mut d = det.lock();
            drain_audio_cmds(&ec, &mut d, Some(&mut ex));
            let mut hit = false;
            ex.push(&mono, |f| hit |= d.push_frame(f));
            ec.audio_score_bits.store(d.last_score().to_bits(), Ordering::Relaxed);
            drop(d);
            if hit {
                ec.signal(Source::Audio);
            }
        };
        let stream = match cfg.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream::<f32, _, _>(
                cfg.config(),
                move |data: &[f32], _: &_| process(&mut data.iter().copied()),
                err_fn,
                None,
            ),
            cpal::SampleFormat::I16 => device.build_input_stream::<i16, _, _>(
                cfg.config(),
                move |data: &[i16], _: &_| process(&mut data.iter().map(|&s| s as f32 / 32768.0)),
                err_fn,
                None,
            ),
            cpal::SampleFormat::I32 => device.build_input_stream::<i32, _, _>(
                cfg.config(),
                move |data: &[i32], _: &_| process(&mut data.iter().map(|&s| s as f32 / 2_147_483_648.0)),
                err_fn,
                None,
            ),
            other => {
                set_audio(&e, "error", &format!("Format audio nesuportat: {other}"));
                thread::sleep(Duration::from_secs(5));
                continue;
            }
        };
        let stream = match stream {
            Ok(s) => s,
            Err(err) => {
                log::warn!("nu pot porni loopback-ul: {err}");
                set_audio(&e, "error", &dev_name);
                thread::sleep(Duration::from_secs(3));
                continue;
            }
        };
        if stream.play().is_err() {
            set_audio(&e, "error", &dev_name);
            thread::sleep(Duration::from_secs(3));
            continue;
        }
        set_audio(&e, "listening", &dev_name);
        // Repornim stream-ul dacă se schimbă dispozitivul implicit (căști ↔ boxe) sau apare o eroare.
        let mut last_check = Instant::now();
        loop {
            thread::sleep(Duration::from_millis(250));
            {
                let score = f32::from_bits(e.audio_score_bits.load(Ordering::Relaxed));
                e.live.lock().audio_score = score;
            }
            if e.shutdown.load(Ordering::Relaxed) || failed.load(Ordering::Relaxed) || !e.audio_enabled.load(Ordering::Relaxed) {
                break;
            }
            if last_check.elapsed() > Duration::from_secs(3) {
                last_check = Instant::now();
                let now_name = cpal::default_host()
                    .default_output_device()
                    .and_then(|d| d.description().ok())
                    .map(|d| d.to_string());
                if now_name.as_deref() != Some(dev_name.as_str()) {
                    break;
                }
            }
        }
        drop(stream);
        set_audio(&e, "off", "");
        thread::sleep(Duration::from_millis(300));
    }
}

fn drain_audio_cmds(e: &Engine, d: &mut AudioDetector, _ex: Option<&mut FeatureExtractor>) {
    let cmds: Vec<AudioCmd> = {
        let Some(mut q) = e.audio_cmds.try_lock() else { return };
        if q.is_empty() {
            return;
        }
        q.drain(..).collect()
    };
    for cmd in cmds {
        match cmd {
            AudioCmd::SetTemplate(tpl, thr) => {
                d.template = tpl;
                d.threshold = thr;
            }
            AudioCmd::SetThreshold(t) => d.threshold = t,
            AudioCmd::Clear => {
                d.clear_samples();
                d.template = None;
                e.audio_samples.store(0, Ordering::Relaxed);
            }
            AudioCmd::TakeSample => {
                let payload = match d.take_sample() {
                    Ok(n) if n >= 2 => match d.build_template() {
                        Ok(tpl) => {
                            let score = tpl.calib_score;
                            let manual = {
                                let mut st = e.store.lock();
                                st.calib.audio = Some(tpl);
                                st.save_calib();
                                st.settings.audio_threshold
                            };
                            if let Some(t) = manual {
                                d.threshold = t;
                            }
                            e.audio_samples.store(0, Ordering::Relaxed);
                            CalibPayload {
                                kind: "audio",
                                ok: true,
                                message: format!("Sunetul cufărului e învățat (potrivire {:.0}%).", score * 100.0),
                                samples: 2,
                                ready: true,
                                score,
                            }
                        }
                        Err(msg) => {
                            d.clear_samples();
                            e.audio_samples.store(0, Ordering::Relaxed);
                            CalibPayload { kind: "audio", ok: false, message: msg.into(), samples: 0, ready: d.template.is_some(), score: 0.0 }
                        }
                    },
                    Ok(n) => {
                        e.audio_samples.store(n as u32, Ordering::Relaxed);
                        CalibPayload {
                            kind: "audio",
                            ok: true,
                            message: "Proba 1 de sunet salvată. Deschide încă un cufăr și apasă din nou.".into(),
                            samples: n,
                            ready: d.template.is_some(),
                            score: 0.0,
                        }
                    }
                    Err(msg) => CalibPayload {
                        kind: "audio",
                        ok: false,
                        message: msg.into(),
                        samples: d.samples_taken(),
                        ready: d.template.is_some(),
                        score: 0.0,
                    },
                };
                e.emit_calib(payload);
            }
        }
    }
}

// ───────────────────────────── Taste ─────────────────────────────

fn key_loop(e: Arc<Engine>) {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    use windows::Win32::UI::Input::XboxController::{XInputGetState, XINPUT_GAMEPAD_X, XINPUT_STATE};

    let mut pad_connected = [false; 4];
    let mut last_pad_scan = Instant::now() - Duration::from_secs(10);
    loop {
        if e.shutdown.load(Ordering::Relaxed) {
            return;
        }
        let vk = e.interact_vk.load(Ordering::Relaxed) as i32;
        let mut down = vk != 0 && unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000 != 0;
        if e.gamepad.load(Ordering::Relaxed) {
            // Controlerele deconectate sunt lente la interogare: le verificăm rar.
            let rescan = last_pad_scan.elapsed() > Duration::from_secs(2);
            if rescan {
                last_pad_scan = Instant::now();
            }
            for i in 0..4u32 {
                if !pad_connected[i as usize] && !rescan {
                    continue;
                }
                let mut st = XINPUT_STATE::default();
                let ok = unsafe { XInputGetState(i, &mut st) } == 0;
                pad_connected[i as usize] = ok;
                if ok && st.Gamepad.wButtons.0 & XINPUT_GAMEPAD_X.0 != 0 {
                    down = true;
                }
            }
        }
        e.key_down.store(down, Ordering::Relaxed);
        if down {
            e.last_key_ms.store(e.mono_ms(), Ordering::Relaxed);
        }
        e.live.lock().key_down = down;
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::prompt::tests::{load_png, scaled, SHOT, SHOT2};

    /// Rulează în CI pe Windows: OCR-ul real din Windows trebuie să citească „SEARCH”
    /// din captura reală a promptului de cufăr.
    #[test]
    fn windows_ocr_reads_search_from_real_screenshot() {
        use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};
        unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED);
        }
        let ocr = match Ocr::new() {
            Ok(o) => o,
            Err(err) => {
                eprintln!("OCR INDISPONIBIL pe acest Windows, testul e sărit: {err}");
                return;
            }
        };
        eprintln!("OCR: limba {}", ocr.lang);
        let words = prompt::parse_words("SEARCH");
        let shot = load_png(SHOT);
        let shot2 = load_png(SHOT2);
        for (name, img) in [("1x", shot.clone()), ("2x", scaled(&shot, 2.0)), ("perete galben", shot2)] {
            let cands = prompt::find_candidates(&img);
            let mut texts = Vec::new();
            let mut found = false;
            for c in cands.iter().take(4) {
                let t0 = Instant::now();
                let text = ocr.read(&prompt::ocr_crop(&img, c.b)).expect("OCR");
                texts.push(format!("{text:?} ({:.1} ms)", t0.elapsed().as_secs_f32() * 1000.0));
                if prompt::match_words(&text, &words).is_some() {
                    found = true;
                    break;
                }
            }
            eprintln!("OCR {name}: {texts:?}");
            assert!(found, "OCR nu a găsit SEARCH la {name}: {texts:?}");
        }

        // Plasa de siguranță: OCR direct pe zona din jurul centrului unui cadru întreg din joc.
        let full = load_png(include_bytes!("../../tests/fixtures/frame_full.png"));
        let (x0, y0, x1, y1) = prompt::SEARCH_REGION.pixels(full.w as u32, full.h as u32);
        let (x0, y0, x1, y1) = (x0 as usize, y0 as usize, x1 as usize, y1 as usize);
        let mut region = prompt::Rgb { w: x1 - x0, h: y1 - y0, px: Vec::new() };
        for y in y0..y1 {
            region.px.extend_from_slice(&full.px[(y * full.w + x0) * 3..(y * full.w + x1) * 3]);
        }
        let area = prompt::center_area(&region);
        let (oimg, scale) = prompt::ocr_area(&region, area);
        let t0 = Instant::now();
        let words = ocr.read_words(&oimg).expect("OCR zonă");
        let ms = t0.elapsed().as_secs_f32() * 1000.0;
        let texts: Vec<&str> = words.iter().map(|w| w.0.as_str()).collect();
        eprintln!("OCR zonă centrală {}×{} (×{scale:.2}, {ms:.0} ms): {texts:?}", oimg.w, oimg.h);
        let hit = words.iter().find(|w| prompt::match_words(&w.0, &words_kw()).is_some());
        let (_, r) = hit.unwrap_or_else(|| panic!("plasa de siguranță nu a găsit SEARCH: {texts:?}"));
        let b = prompt::BoxI {
            x: area.x + (r.X / scale) as i32,
            y: area.y + (r.Y / scale) as i32,
            w: (r.Width / scale) as i32,
            h: (r.Height / scale) as i32,
        };
        let gold = prompt::gold_fraction(&region, b);
        eprintln!("plasa de siguranță: cutie {b:?}, auriu {:.0}%", gold * 100.0);
        assert!(gold >= prompt::GOLD_MIN, "auriu {gold}");
    }

    fn words_kw() -> Vec<String> {
        prompt::parse_words("SEARCH")
    }
}
