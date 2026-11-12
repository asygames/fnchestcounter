//! Windows: captura (Windows Graphics Capture), sunetul (WASAPI loopback) și tasta de interacțiune.
//!
//! Toate sunt doar CITIRI, exact ca OBS / Discord: nu injectăm nimic în joc,
//! nu citim memoria lui și nu trimitem taste.

use crate::detect::audio::{AudioDetector, FeatureExtractor};
use crate::detect::fusion::Source;
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
        let interval = Duration::from_millis(1000 / cfg.fps.max(1) as u64);
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

        self.frames += 1;
        let el = self.fps_t0.elapsed();
        let analyze_ms = t0.elapsed().as_secs_f32() * 1000.0;
        {
            let mut l = e.live.lock();
            l.analyze_ms = l.analyze_ms * 0.9 + analyze_ms * 0.1;
            if el >= Duration::from_secs(1) {
                l.fps = self.frames as f32 / el.as_secs_f32();
                self.frames = 0;
                self.fps_t0 = Instant::now();
            }
        }
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
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
