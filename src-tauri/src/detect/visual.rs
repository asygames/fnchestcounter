//! Detecția vizuală a promptului de „Caută” al cufărului.
//!
//! Ideea: jucătorul face 3+ probe (F8) cu promptul cufărului pe ecran, la cufere
//! din locuri diferite. Pixelii care rămân la fel în toate probele sunt UI-ul
//! promptului (text, iconiță, tastă); cei care se schimbă sunt fundalul jocului.
//! Din ei facem o mască, iar la rulare calculăm corelația normalizată (NCC)
//! doar pe pixelii din mască, cu o mică toleranță de deplasare.
//!
//! Totul lucrează pe o imagine gri micșorată la o lățime fixă, deci costul
//! nu depinde de rezoluția jocului (1080p sau 4K).

use serde::{Deserialize, Serialize};

/// Lățimea de lucru a zonei analizate (pixeli după micșorare).
pub const WORK_W: usize = 192;
/// Deplasarea maximă căutată (în pixeli de lucru), pentru prompturi care se mișcă puțin.
pub const MAX_SHIFT: i32 = 6;
/// Pragul de variație (0..1) sub care un pixel e considerat „UI stabil”.
const STABLE_STD: f32 = 0.07;
/// Gradientul minim (în fiecare probă) pentru o „margine de UI”.
const EDGE_GRAD: f32 = 0.25;

/// Imagine gri, valori 0..1.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Gray {
    pub w: usize,
    pub h: usize,
    pub px: Vec<f32>,
}

impl Gray {
    pub fn new(w: usize, h: usize) -> Self {
        Self { w, h, px: vec![0.0; w * h] }
    }

    #[inline]
    pub fn at(&self, x: usize, y: usize) -> f32 {
        self.px[y * self.w + x]
    }

    /// Imaginea ca octeți 0..255 (pentru previzualizare în UI).
    pub fn to_u8(&self) -> Vec<u8> {
        self.px.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8).collect()
    }
}

/// Zona de interes, ca fracțiuni din cadru (0..1).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Roi {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Default for Roi {
    fn default() -> Self {
        // Promptul de interacțiune apare lângă centrul ecranului, puțin dedesubt.
        Self { x0: 0.38, y0: 0.50, x1: 0.70, y1: 0.72 }
    }
}

impl Roi {
    pub fn sanitized(self) -> Self {
        let c = |v: f32| v.clamp(0.0, 1.0);
        let (mut x0, mut x1) = (c(self.x0.min(self.x1)), c(self.x0.max(self.x1)));
        let (mut y0, mut y1) = (c(self.y0.min(self.y1)), c(self.y0.max(self.y1)));
        if x1 - x0 < 0.03 {
            x1 = (x0 + 0.03).min(1.0);
            x0 = x1 - 0.03;
        }
        if y1 - y0 < 0.03 {
            y1 = (y0 + 0.03).min(1.0);
            y0 = y1 - 0.03;
        }
        Self { x0, y0, x1, y1 }
    }

    /// Dreptunghiul în pixeli pentru un cadru de mărimea dată: (x0, y0, x1, y1).
    pub fn pixels(&self, fw: u32, fh: u32) -> (u32, u32, u32, u32) {
        let r = self.sanitized();
        let x0 = ((r.x0 * fw as f32) as u32).min(fw.saturating_sub(2));
        let y0 = ((r.y0 * fh as f32) as u32).min(fh.saturating_sub(2));
        let x1 = ((r.x1 * fw as f32) as u32).clamp(x0 + 1, fw);
        let y1 = ((r.y1 * fh as f32) as u32).clamp(y0 + 1, fh);
        (x0, y0, x1, y1)
    }

    /// Înălțimea de lucru, păstrând proporția zonei pe un cadru dat.
    pub fn work_h(&self, fw: u32, fh: u32) -> usize {
        let (x0, y0, x1, y1) = self.pixels(fw, fh);
        let w = (x1 - x0).max(1) as f32;
        let h = (y1 - y0).max(1) as f32;
        ((WORK_W as f32 * h / w).round() as usize).clamp(16, 256)
    }
}

/// Micșorează un buffer RGBA/BGRA (cu row pitch) la `out_w × out_h` gri, prin medie pe blocuri.
/// `bgra = true` dacă ordinea e B,G,R,A.
pub fn downscale_gray(
    buf: &[u8],
    w: usize,
    h: usize,
    pitch: usize,
    bgra: bool,
    out_w: usize,
    out_h: usize,
) -> Gray {
    let mut out = Gray::new(out_w, out_h);
    if w == 0 || h == 0 || out_w == 0 || out_h == 0 {
        return out;
    }
    let (ri, bi) = if bgra { (2, 0) } else { (0, 2) };
    // Pentru viteză eșantionăm cel mult 4×4 pixeli din fiecare bloc.
    for oy in 0..out_h {
        let sy0 = oy * h / out_h;
        let sy1 = ((oy + 1) * h / out_h).max(sy0 + 1).min(h);
        let ystep = ((sy1 - sy0) / 4).max(1);
        for ox in 0..out_w {
            let sx0 = ox * w / out_w;
            let sx1 = ((ox + 1) * w / out_w).max(sx0 + 1).min(w);
            let xstep = ((sx1 - sx0) / 4).max(1);
            let mut acc = 0u32;
            let mut n = 0u32;
            let mut y = sy0;
            while y < sy1 {
                let row = y * pitch;
                let mut x = sx0;
                while x < sx1 {
                    let i = row + x * 4;
                    if i + 2 < buf.len() {
                        // Luminanță aproximativă (Rec. 601) în întregi.
                        acc += 77 * buf[i + ri] as u32 + 150 * buf[i + 1] as u32 + 29 * buf[i + bi] as u32;
                        n += 1;
                    }
                    x += xstep;
                }
                y += ystep;
            }
            if n > 0 {
                out.px[oy * out_w + ox] = acc as f32 / (n as f32 * 256.0 * 255.0);
            }
        }
    }
    out
}

/// Șablonul promptului: pixelii din mască și valorile lor medii.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Template {
    pub w: usize,
    pub h: usize,
    /// Indici (în imaginea de lucru) ai pixelilor din mască.
    pub idx: Vec<u32>,
    /// Valorile șablonului pe acei pixeli, deja centrate (medie 0) și normate (normă 1).
    pub val: Vec<f32>,
    /// Imaginea medie (pentru previzualizare).
    pub mean: Gray,
    /// Masca completă 0/1 (pentru previzualizare).
    pub mask: Vec<u8>,
    /// Scorul minim obținut la validarea „lasă-una-afară” la calibrare.
    pub calib_score: f32,
}

#[derive(Debug, PartialEq)]
pub enum CalibError {
    NotEnoughSamples,
    SizeMismatch,
    MaskTooSmall,
    MaskTooLarge,
}

impl std::fmt::Display for CalibError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            CalibError::NotEnoughSamples => "Ai nevoie de cel puțin 3 probe.",
            CalibError::SizeMismatch => "Probele au mărimi diferite (s-a schimbat rezoluția?). Șterge probele și reia.",
            CalibError::MaskTooSmall => "Nu am găsit un prompt comun în probe. Verifică zona și să fie promptul cufărului pe ecran la fiecare probă.",
            CalibError::MaskTooLarge => "Fundalul e aproape identic în probe. Fă probele la cufere din locuri diferite.",
        };
        f.write_str(s)
    }
}

/// Construiește șablonul din probe (minimum 3).
pub fn build_template(samples: &[Gray]) -> Result<Template, CalibError> {
    if samples.len() < 3 {
        return Err(CalibError::NotEnoughSamples);
    }
    let (w, h) = (samples[0].w, samples[0].h);
    if samples.iter().any(|s| s.w != w || s.h != h) {
        return Err(CalibError::SizeMismatch);
    }
    let n = samples.len() as f32;
    let mut mean = Gray::new(w, h);
    let mut std = vec![0f32; w * h];
    for (i, sd) in std.iter_mut().enumerate() {
        let m = samples.iter().map(|s| s.px[i]).sum::<f32>() / n;
        let v = samples.iter().map(|s| (s.px[i] - m).powi(2)).sum::<f32>() / n;
        mean.px[i] = m;
        *sd = v.sqrt();
    }
    // Dacă aproape toată imaginea e identică în probe, fundalul nu s-a schimbat
    // și nu putem separa promptul de decor.
    let stable_all = std.iter().filter(|&&v| v < STABLE_STD).count();
    if stable_all > w * h * 85 / 100 {
        return Err(CalibError::MaskTooLarge);
    }
    // 1) Margini „de UI”: contrast local puternic în TOATE probele, în același loc.
    //    Fundalul jocului se schimbă de la o probă la alta, deci marginile lui nu se repetă.
    let m = MAX_SHIFT as usize;
    let mut ui_edge = vec![0u8; w * h];
    let mut edge_count = 0usize;
    for y in (m + 1)..h.saturating_sub(m + 1) {
        for x in (m + 1)..w.saturating_sub(m + 1) {
            let i = y * w + x;
            if std[i] >= STABLE_STD {
                continue;
            }
            let min_grad = samples
                .iter()
                .map(|s| {
                    let gx = s.at(x + 1, y) - s.at(x - 1, y);
                    let gy = s.at(x, y + 1) - s.at(x, y - 1);
                    (gx * gx + gy * gy).sqrt()
                })
                .fold(f32::MAX, f32::min);
            if min_grad > EDGE_GRAD {
                ui_edge[i] = 1;
                edge_count += 1;
            }
        }
    }
    // 2) Masca = pixeli stabili aflați aproape de o margine de UI (raza 3).
    //    Așa prindem și interiorul literelor/iconițelor, dar nu cerul uniform.
    let mut clean = vec![0u8; w * h];
    let r = 3i32;
    for y in m..h.saturating_sub(m) {
        for x in m..w.saturating_sub(m) {
            let i = y * w + x;
            if std[i] >= STABLE_STD {
                continue;
            }
            'near: for dy in -r..=r {
                for dx in -r..=r {
                    let (xx, yy) = (x as i32 + dx, y as i32 + dy);
                    if xx >= 0 && yy >= 0 && (xx as usize) < w && (yy as usize) < h && ui_edge[yy as usize * w + xx as usize] == 1 {
                        clean[i] = 1;
                        break 'near;
                    }
                }
            }
        }
    }
    let idx: Vec<u32> = (0..w * h).filter(|&i| clean[i] == 1).map(|i| i as u32).collect();
    if edge_count < (w * h / 400).max(30) {
        return Err(CalibError::MaskTooSmall);
    }
    let total = w * h;
    let contrast = local_contrast(&mean, &idx);
    if idx.len() < total / 60 || contrast < 0.04 {
        return Err(CalibError::MaskTooSmall);
    }
    if idx.len() > total * 7 / 10 {
        return Err(CalibError::MaskTooLarge);
    }
    let val = normalized(&idx.iter().map(|&i| mean.px[i as usize]).collect::<Vec<_>>());
    let mut tpl = Template { w, h, idx, val, mean, mask: clean, calib_score: 0.0 };

    // Validare: scorul fiecărei probe față de șablon (aproximare a lasă-una-afară).
    let mut min_score = f32::MAX;
    for s in samples {
        min_score = min_score.min(match_score(&tpl, s).0);
    }
    tpl.calib_score = min_score;
    Ok(tpl)
}

fn local_contrast(img: &Gray, idx: &[u32]) -> f32 {
    if idx.is_empty() {
        return 0.0;
    }
    let vals: Vec<f32> = idx.iter().map(|&i| img.px[i as usize]).collect();
    let m = vals.iter().sum::<f32>() / vals.len() as f32;
    (vals.iter().map(|v| (v - m).powi(2)).sum::<f32>() / vals.len() as f32).sqrt()
}

fn normalized(v: &[f32]) -> Vec<f32> {
    let m = v.iter().sum::<f32>() / v.len().max(1) as f32;
    let mut out: Vec<f32> = v.iter().map(|x| x - m).collect();
    let norm = out.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-6 {
        out.iter_mut().for_each(|x| *x /= norm);
    }
    out
}

/// Scorul NCC (−1..1) cel mai bun pe deplasările ±MAX_SHIFT și deplasarea găsită.
pub fn match_score(tpl: &Template, img: &Gray) -> (f32, (i32, i32)) {
    if tpl.idx.is_empty() || img.w != tpl.w || img.h != tpl.h {
        return (0.0, (0, 0));
    }
    let w = img.w as i32;
    let h = img.h as i32;
    let mut best = (-1.0f32, (0, 0));
    let mut buf = vec![0f32; tpl.idx.len()];
    // Căutare grosieră din 2 în 2, apoi rafinare în jurul celei mai bune poziții.
    let mut eval = |dx: i32, dy: i32, best: &mut (f32, (i32, i32))| {
        let mut sum = 0f32;
        for (k, &i) in tpl.idx.iter().enumerate() {
            let x = (i as i32 % w) + dx;
            let y = (i as i32 / w) + dy;
            let v = if x >= 0 && y >= 0 && x < w && y < h { img.px[(y * w + x) as usize] } else { 0.0 };
            buf[k] = v;
            sum += v;
        }
        let m = sum / buf.len() as f32;
        let mut dot = 0f32;
        let mut nn = 0f32;
        for (k, &t) in tpl.val.iter().enumerate() {
            let d = buf[k] - m;
            dot += d * t;
            nn += d * d;
        }
        let s = if nn > 1e-8 { dot / nn.sqrt() } else { 0.0 };
        if s > best.0 {
            *best = (s, (dx, dy));
        }
    };
    let mut dy = -MAX_SHIFT;
    while dy <= MAX_SHIFT {
        let mut dx = -MAX_SHIFT;
        while dx <= MAX_SHIFT {
            eval(dx, dy, &mut best);
            dx += 2;
        }
        dy += 2;
    }
    let (cx, cy) = best.1;
    for dy in (cy - 1)..=(cy + 1) {
        for dx in (cx - 1)..=(cx + 1) {
            if (dx - cx).abs() + (dy - cy).abs() == 0 || dx.abs() > MAX_SHIFT || dy.abs() > MAX_SHIFT {
                continue;
            }
            eval(dx, dy, &mut best);
        }
    }
    best
}

/// Starea promptului cu histerezis, plus logica „cufăr deschis”.
#[derive(Debug)]
pub struct PromptTracker {
    pub on_threshold: f32,
    pub off_threshold: f32,
    visible: bool,
    on_frames: u32,
    off_frames: u32,
    visible_since_ms: u64,
    key_seen_during_visible: bool,
}

impl PromptTracker {
    pub fn new(on_threshold: f32) -> Self {
        Self {
            on_threshold,
            off_threshold: on_threshold - 0.15,
            visible: false,
            on_frames: 0,
            off_frames: 0,
            visible_since_ms: 0,
            key_seen_during_visible: false,
        }
    }

    pub fn set_threshold(&mut self, on: f32) {
        self.on_threshold = on;
        self.off_threshold = on - 0.15;
    }

    pub fn visible(&self) -> bool {
        self.visible
    }

    /// Procesează un cadru. `key_recent` = tasta de interacțiune a fost ținută
    /// apăsată recent (în ultimele ~700 ms). `require_key` = numărăm doar cu tastă.
    /// Întoarce `true` dacă tocmai s-a deschis un cufăr.
    pub fn update(&mut self, score: f32, now_ms: u64, key_recent: bool, require_key: bool) -> bool {
        if !self.visible {
            if score >= self.on_threshold {
                self.on_frames += 1;
                if self.on_frames >= 2 {
                    self.visible = true;
                    self.visible_since_ms = now_ms;
                    self.key_seen_during_visible = key_recent;
                    self.off_frames = 0;
                }
            } else {
                self.on_frames = 0;
            }
            return false;
        }
        if key_recent {
            self.key_seen_during_visible = true;
        }
        if score < self.off_threshold {
            self.off_frames += 1;
            if self.off_frames >= 2 {
                self.visible = false;
                self.on_frames = 0;
                let shown_for = now_ms.saturating_sub(self.visible_since_ms);
                let opened = if require_key {
                    self.key_seen_during_visible && key_recent
                } else {
                    // Fără tastă: promptul trebuie să fi stat măcar puțin, dar nu foarte mult.
                    (150..20_000).contains(&shown_for)
                };
                self.key_seen_during_visible = false;
                return opened;
            }
        } else {
            self.off_frames = 0;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Generator pseudo-aleator determinist (fără dependențe).
    struct Rng(u64);
    impl Rng {
        fn f(&mut self) -> f32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 % 10_000) as f32 / 10_000.0
        }
    }

    /// Fundal „de joc”: gradient + pete mari + zgomot, diferit pentru fiecare seed.
    fn background(seed: u64, w: usize, h: usize) -> Gray {
        let mut r = Rng(seed * 7919 + 13);
        let mut g = Gray::new(w, h);
        let (a, b, c) = (r.f(), r.f(), r.f());
        let blobs: Vec<(f32, f32, f32, f32)> = (0..6).map(|_| (r.f() * w as f32, r.f() * h as f32, 8.0 + r.f() * 30.0, r.f())).collect();
        for y in 0..h {
            for x in 0..w {
                let mut v = 0.2 + 0.5 * (a * x as f32 / w as f32 + b * y as f32 / h as f32) * c;
                for &(bx, by, br, bv) in &blobs {
                    let d = ((x as f32 - bx).powi(2) + (y as f32 - by).powi(2)).sqrt();
                    if d < br {
                        v = v * 0.4 + bv * 0.6;
                    }
                }
                v += (r.f() - 0.5) * 0.08;
                g.px[y * w + x] = v.clamp(0.0, 1.0);
            }
        }
        g
    }

    /// Desenează un „prompt” sintetic: panou închis cu un pătrat-tastă și bare albe (text).
    fn draw_prompt(img: &mut Gray, ox: i32, oy: i32, variant: u8) {
        let w = img.w as i32;
        let mut put = |x: i32, y: i32, v: f32| {
            if x >= 0 && y >= 0 && x < w && y < img.h as i32 {
                img.px[(y * w + x) as usize] = v;
            }
        };
        for y in 0..22 {
            for x in 0..90 {
                put(ox + x, oy + y, 0.08);
            }
        }
        for y in 4..18 {
            for x in 4..18 {
                let border = y == 4 || y == 17 || x == 4 || x == 17;
                put(ox + x, oy + y, if border { 0.95 } else { 0.25 });
            }
        }
        // „Text”: bare albe; varianta 1 are alt text (alt tip de container).
        let bars: &[(i32, i32)] = if variant == 0 { &[(24, 40), (44, 52), (56, 84)] } else { &[(24, 30), (34, 70), (74, 80)] };
        for &(x0, x1) in bars {
            for y in 8..14 {
                for x in x0..x1 {
                    put(ox + x, oy + y, 0.97);
                }
            }
        }
    }

    const H: usize = 80;

    #[test]
    fn calibrates_and_detects_prompt_on_new_backgrounds() {
        let samples: Vec<Gray> = (0..4)
            .map(|s| {
                let mut g = background(s, WORK_W, H);
                draw_prompt(&mut g, 50, 30, 0);
                g
            })
            .collect();
        let tpl = build_template(&samples).expect("calibrare");
        assert!(tpl.calib_score > 0.8, "scor calibrare {}", tpl.calib_score);

        let mut hits = 0;
        let mut false_pos = 0;
        let mut other_container_max = 0f32;
        for s in 100..140 {
            let bg = background(s, WORK_W, H);
            let mut with = bg.clone();
            // deplasare mică, ca și cum promptul s-ar mișca puțin
            draw_prompt(&mut with, 50 + (s as i32 % 5) - 2, 30 + (s as i32 % 3) - 1, 0);
            let mut other = bg.clone();
            draw_prompt(&mut other, 50, 30, 1);
            let (sw, _) = match_score(&tpl, &with);
            let (sn, _) = match_score(&tpl, &bg);
            let (so, _) = match_score(&tpl, &other);
            other_container_max = other_container_max.max(so);
            if sw > 0.6 {
                hits += 1;
            }
            if sn > 0.6 {
                false_pos += 1;
            }
        }
        assert_eq!(hits, 40, "prompt nedetectat");
        assert_eq!(false_pos, 0, "fals pozitiv pe fundal");
        assert!(other_container_max < 0.85, "alt container prea asemănător: {other_container_max}");
    }

    #[test]
    fn rejects_bad_calibration() {
        let same: Vec<Gray> = (0..3).map(|_| background(1, WORK_W, H)).collect();
        assert_eq!(build_template(&same).unwrap_err(), CalibError::MaskTooLarge);
        let noise: Vec<Gray> = (0..3).map(|s| background(s + 50, WORK_W, H)).collect();
        assert!(build_template(&noise).is_err());
        assert_eq!(build_template(&noise[..2]).unwrap_err(), CalibError::NotEnoughSamples);
    }

    #[test]
    fn tracker_counts_only_with_key() {
        let mut t = PromptTracker::new(0.6);
        let mut now = 0;
        let mut step = |t: &mut PromptTracker, s: f32, key: bool| {
            now += 66;
            t.update(s, now, key, true)
        };
        // Apare promptul, jucătorul pleacă fără să apese: nu numără.
        for _ in 0..10 {
            assert!(!step(&mut t, 0.9, false));
        }
        let mut opened = false;
        for _ in 0..3 {
            opened |= step(&mut t, 0.1, false);
        }
        assert!(!opened);
        // Apare promptul, ține tasta, promptul dispare: numără o dată.
        for _ in 0..10 {
            step(&mut t, 0.9, false);
        }
        for _ in 0..5 {
            step(&mut t, 0.85, true);
        }
        let mut count = 0;
        for _ in 0..6 {
            if step(&mut t, 0.1, true) {
                count += 1;
            }
        }
        assert_eq!(count, 1);
    }

    #[test]
    fn downscale_handles_pitch_and_bgra() {
        // 8×4 BGRA cu pitch 40: jumătatea stângă albă, dreapta neagră.
        let (w, h, pitch) = (8usize, 4usize, 40usize);
        let mut buf = vec![0u8; pitch * h];
        for y in 0..h {
            for x in 0..4 {
                let i = y * pitch + x * 4;
                buf[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
        let g = downscale_gray(&buf, w, h, pitch, true, 2, 1);
        assert!(g.px[0] > 0.95 && g.px[1] < 0.05, "{:?}", g.px);
    }
}
