//! Detecția audio a sunetului de deschidere a cufărului.
//!
//! Semnalul (loopback din placa de sunet) e transformat în benzi de energie
//! logaritmice (un fel de spectrogramă mel simplificată), la ~94 cadre/s.
//! Șablonul e o fereastră de ~0,7 s din jurul momentului deschiderii, luată la
//! calibrare (F7 imediat după ce deschizi un cufăr). La rulare corelăm ultima
//! fereastră cu șablonul; peste prag = cufăr deschis.

use rustfft::{num_complex::Complex, Fft, FftPlanner};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;

pub const FFT: usize = 1024;
pub const BANDS: usize = 28;
/// Lungimea șablonului în cadre (~0,7 s la 48 kHz cu pas 512).
pub const TPL_FRAMES: usize = 64;
/// Cât istoric păstrăm pentru calibrare (~4 s).
const HISTORY: usize = 400;

/// Transformă eșantioane mono în cadre de benzi (log-energie).
pub struct FeatureExtractor {
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    hop: usize,
    pending: Vec<f32>,
    band_edges: Vec<(usize, usize)>,
    scratch: Vec<Complex<f32>>,
    pub sample_rate: u32,
}

impl FeatureExtractor {
    pub fn new(sample_rate: u32) -> Self {
        let fft = FftPlanner::new().plan_fft_forward(FFT);
        let window = (0..FFT)
            .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / FFT as f32).cos())
            .collect();
        // Benzi logaritmice între 120 Hz și 9 kHz.
        let bin_hz = sample_rate as f32 / FFT as f32;
        let (lo, hi) = (120f32, 9000f32.min(sample_rate as f32 / 2.0 - bin_hz));
        let mut band_edges = Vec::with_capacity(BANDS);
        for b in 0..BANDS {
            let f0 = lo * (hi / lo).powf(b as f32 / BANDS as f32);
            let f1 = lo * (hi / lo).powf((b + 1) as f32 / BANDS as f32);
            let i0 = (f0 / bin_hz).floor() as usize;
            let i1 = ((f1 / bin_hz).ceil() as usize).max(i0 + 1);
            band_edges.push((i0, i1.min(FFT / 2)));
        }
        // Pasul (hop) e ~10,7 ms indiferent de rată.
        let hop = ((sample_rate as f32 * 0.010_67).round() as usize).clamp(128, FFT);
        Self {
            fft,
            window,
            hop,
            pending: Vec::with_capacity(FFT * 4),
            band_edges,
            scratch: vec![Complex::new(0.0, 0.0); FFT],
            sample_rate,
        }
    }

    /// Adaugă eșantioane mono; apelează `on_frame` pentru fiecare cadru nou.
    pub fn push(&mut self, mono: &[f32], mut on_frame: impl FnMut([f32; BANDS])) {
        self.pending.extend_from_slice(mono);
        while self.pending.len() >= FFT {
            for i in 0..FFT {
                self.scratch[i] = Complex::new(self.pending[i] * self.window[i], 0.0);
            }
            self.fft.process(&mut self.scratch);
            let mut bands = [0f32; BANDS];
            for (b, &(i0, i1)) in self.band_edges.iter().enumerate() {
                let mut e = 0f32;
                for c in &self.scratch[i0..i1] {
                    e += c.norm_sqr();
                }
                bands[b] = (e / (i1 - i0) as f32 + 1e-9).log10();
            }
            on_frame(bands);
            self.pending.drain(..self.hop);
        }
    }
}

/// „Volumul” unui cadru de prim-plan: cât iese sunetul peste fundal, însumat pe benzi.
fn frame_energy(f: &[f32; BANDS]) -> f32 {
    f.iter().sum()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AudioTemplate {
    /// TPL_FRAMES × BANDS, centrat și normat (normă 1).
    pub val: Vec<f32>,
    /// Energia medie a șablonului (log); o folosim ca poartă de volum.
    pub energy: f32,
    /// Scorul obținut între probe la calibrare.
    pub calib_score: f32,
    pub samples: u32,
}

fn normalize(v: &mut [f32]) {
    let m = v.iter().sum::<f32>() / v.len().max(1) as f32;
    v.iter_mut().for_each(|x| *x -= m);
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > 1e-6 {
        v.iter_mut().for_each(|x| *x /= n);
    }
}

fn corr(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Detectorul: ține istoricul, face calibrarea și potrivirea.
pub struct AudioDetector {
    history: VecDeque<[f32; BANDS]>,
    pub template: Option<AudioTemplate>,
    pending_samples: Vec<Vec<f32>>,
    pub threshold: f32,
    refractory_until: u64,
    frame_no: u64,
    last_score: f32,
    peak: f32,
    peak_rising: bool,
    window_buf: Vec<f32>,
    /// Nivelul de fundal urmărit pe fiecare bandă (log-energie).
    floor: [f32; BANDS],
    floor_ready: bool,
}

impl AudioDetector {
    pub fn new(template: Option<AudioTemplate>, threshold: f32) -> Self {
        Self {
            history: VecDeque::with_capacity(HISTORY + 1),
            template,
            pending_samples: Vec::new(),
            threshold,
            refractory_until: 0,
            frame_no: 0,
            last_score: 0.0,
            peak: 0.0,
            peak_rising: false,
            window_buf: vec![0.0; TPL_FRAMES * BANDS],
            floor: [0.0; BANDS],
            floor_ready: false,
        }
    }

    pub fn last_score(&self) -> f32 {
        self.last_score
    }

    pub fn samples_taken(&self) -> usize {
        self.pending_samples.len()
    }

    /// Procesează un cadru nou. Întoarce `true` la detecție.
    pub fn push_frame(&mut self, raw: [f32; BANDS]) -> bool {
        // Scădem fundalul (muzică, vânt, ambient): rămâne doar ce e nou și mai tare.
        if !self.floor_ready {
            self.floor = raw;
            self.floor_ready = true;
        }
        let mut f = [0f32; BANDS];
        for b in 0..BANDS {
            let v = raw[b];
            let nf = &mut self.floor[b];
            if v < *nf {
                *nf += 0.2 * (v - *nf); // coboară repede
            } else {
                *nf += 0.004 * (v - *nf); // urcă încet (~2,5 s)
            }
            f[b] = (v - *nf - 0.15).max(0.0);
        }
        self.history.push_back(f);
        if self.history.len() > HISTORY {
            self.history.pop_front();
        }
        self.frame_no += 1;
        let Some(tpl) = &self.template else {
            self.last_score = 0.0;
            return false;
        };
        if self.history.len() < TPL_FRAMES {
            return false;
        }
        let start = self.history.len() - TPL_FRAMES;
        let mut energy = 0f32;
        for (k, fr) in self.history.range(start..).enumerate() {
            self.window_buf[k * BANDS..(k + 1) * BANDS].copy_from_slice(fr);
            energy += frame_energy(fr);
        }
        energy /= TPL_FRAMES as f32;
        // Poartă de volum: mult mai încet decât șablonul = nu e cufărul.
        let score = if energy < tpl.energy * 0.3 {
            0.0
        } else {
            normalize(&mut self.window_buf);
            corr(&self.window_buf, &tpl.val)
        };
        self.last_score = score;

        // Detectăm vârful peste prag (nu fiecare cadru peste prag).
        let mut hit = false;
        if score >= self.threshold {
            if score > self.peak {
                self.peak = score;
                self.peak_rising = true;
            } else if self.peak_rising && score < self.peak - 0.02 {
                self.peak_rising = false;
                if self.frame_no >= self.refractory_until {
                    hit = true;
                    // ~1,5 s fără o nouă detecție.
                    self.refractory_until = self.frame_no + 140;
                }
            }
        } else {
            if self.peak_rising && self.frame_no >= self.refractory_until {
                hit = true;
                self.refractory_until = self.frame_no + 140;
            }
            self.peak = 0.0;
            self.peak_rising = false;
        }
        hit
    }

    /// Ia o probă din ultimele ~3,5 s: fereastra care începe la cel mai puternic atac (onset).
    /// Întoarce numărul de probe strânse.
    pub fn take_sample(&mut self) -> Result<usize, &'static str> {
        if self.history.len() < TPL_FRAMES + 8 {
            return Err("Nu am destul sunet înregistrat încă. Mai așteaptă o secundă.");
        }
        let h: Vec<[f32; BANDS]> = self.history.iter().copied().collect();
        let energies: Vec<f32> = h.iter().map(frame_energy).collect();
        let noise = percentile(&energies, 0.2);
        let mut best = (f32::MIN, 0usize);
        for s in 3..h.len() - TPL_FRAMES {
            // Onset = creștere pozitivă de energie pe benzi (spectral flux) la începutul ferestrei.
            let mut flux = 0f32;
            for k in 0..3 {
                for (cur, prev) in h[s + k].iter().zip(&h[s + k - 1]) {
                    flux += (cur - prev).max(0.0);
                }
            }
            let loud: f32 = energies[s..s + TPL_FRAMES / 3].iter().sum::<f32>() / (TPL_FRAMES / 3) as f32;
            let v = flux + 4.0 * (loud - noise);
            if v > best.0 {
                best = (v, s);
            }
        }
        let s = best.1.saturating_sub(2);
        let loud: f32 = energies[s..s + TPL_FRAMES].iter().sum::<f32>() / TPL_FRAMES as f32;
        if loud - noise < 1.0 {
            return Err("N-am auzit niciun sunet clar. Dă volumul jocului mai tare și apasă imediat după ce deschizi cufărul.");
        }
        let mut v = Vec::with_capacity(TPL_FRAMES * BANDS);
        for fr in &h[s..s + TPL_FRAMES] {
            v.extend_from_slice(fr);
        }
        self.pending_samples.push(v);
        Ok(self.pending_samples.len())
    }

    pub fn clear_samples(&mut self) {
        self.pending_samples.clear();
    }

    /// Construiește șablonul din probe (minimum 2), aliniindu-le între ele.
    pub fn build_template(&mut self) -> Result<AudioTemplate, &'static str> {
        if self.pending_samples.len() < 2 {
            return Err("Ai nevoie de cel puțin 2 probe de sunet.");
        }
        let tpl = build_audio_template(&self.pending_samples)?;
        self.template = Some(tpl.clone());
        // Prag automat: 80% din potrivirea dintre probe, între 0,45 și 0,85.
        self.threshold = (tpl.calib_score * 0.8).clamp(0.45, 0.85);
        self.pending_samples.clear();
        Ok(tpl)
    }
}

fn percentile(v: &[f32], p: f32) -> f32 {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    s[((s.len() - 1) as f32 * p) as usize]
}

/// Șablon din probe brute (TPL_FRAMES × BANDS fiecare).
pub fn build_audio_template(samples: &[Vec<f32>]) -> Result<AudioTemplate, &'static str> {
    if samples.len() < 2 {
        return Err("Ai nevoie de cel puțin 2 probe de sunet.");
    }
    let normed: Vec<Vec<f32>> = samples
        .iter()
        .map(|s| {
            let mut v = s.clone();
            normalize(&mut v);
            v
        })
        .collect();
    // Potrivirea între fiecare pereche de probe.
    let mut min_pair = f32::MAX;
    for i in 0..normed.len() {
        for j in i + 1..normed.len() {
            min_pair = min_pair.min(corr(&normed[i], &normed[j]));
        }
    }
    if min_pair < 0.35 {
        return Err("Probele de sunet nu seamănă între ele. Reia: deschide un cufăr și apasă imediat tasta de probă.");
    }
    let mut avg = vec![0f32; TPL_FRAMES * BANDS];
    let mut energy = 0f32;
    for s in samples {
        for (a, v) in avg.iter_mut().zip(s) {
            *a += v / samples.len() as f32;
        }
        for k in 0..TPL_FRAMES {
            let mut fr = [0f32; BANDS];
            fr.copy_from_slice(&s[k * BANDS..(k + 1) * BANDS]);
            energy += frame_energy(&fr);
        }
    }
    energy /= (TPL_FRAMES * samples.len()) as f32;
    normalize(&mut avg);
    Ok(AudioTemplate { val: avg, energy, calib_score: min_pair, samples: samples.len() as u32 })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rng(u64);
    impl Rng {
        fn f(&mut self) -> f32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 % 100_000) as f32 / 100_000.0 - 0.5
        }
    }

    const SR: u32 = 48_000;

    /// „Sunetul cufărului”: un clinchet (arpegiu) cu atac rapid și scădere.
    fn chest_sound(out: &mut [f32], at: usize, gain: f32) {
        let notes = [880.0, 1318.5, 1760.0, 2637.0];
        for (n, f) in notes.iter().enumerate() {
            let start = at + n * (SR as usize / 14);
            for i in 0..(SR as usize / 3) {
                if start + i >= out.len() {
                    break;
                }
                let t = i as f32 / SR as f32;
                let env = (-t * 9.0).exp() * (t * 400.0).min(1.0);
                out[start + i] += gain * env * (2.0 * std::f32::consts::PI * f * t).sin();
            }
        }
    }

    /// Alt sunet (pași/împușcături): zgomot în rafale.
    fn gunshot(out: &mut [f32], at: usize, rng: &mut Rng) {
        for i in 0..(SR as usize / 8) {
            if at + i >= out.len() {
                break;
            }
            let t = i as f32 / SR as f32;
            out[at + i] += 0.6 * (-t * 30.0).exp() * rng.f();
        }
    }

    fn run(det: &mut AudioDetector, ex: &mut FeatureExtractor, sig: &[f32]) -> Vec<usize> {
        let mut hits = Vec::new();
        let mut frame = 0usize;
        for chunk in sig.chunks(480) {
            ex.push(chunk, |f| {
                frame += 1;
                if det.push_frame(f) {
                    hits.push(frame);
                }
            });
        }
        hits
    }

    #[test]
    fn calibrates_and_detects_chest_sound_in_noise() {
        let mut rng = Rng(42);
        let mut ex = FeatureExtractor::new(SR);
        let mut det = AudioDetector::new(None, 0.6);

        // Calibrare: 2 probe, fiecare cu muzică/zgomot de fundal diferit.
        for k in 0..2 {
            let mut sig: Vec<f32> = (0..SR as usize * 3).map(|_| rng.f() * 0.03).collect();
            for (i, s) in sig.iter_mut().enumerate() {
                *s += 0.02 * (2.0 * std::f32::consts::PI * (220.0 + k as f32 * 30.0) * i as f32 / SR as f32).sin();
            }
            chest_sound(&mut sig, SR as usize + k * 1000, 0.5);
            run(&mut det, &mut ex, &sig);
            det.take_sample().expect("probă");
        }
        let tpl = det.build_template().expect("șablon");
        assert!(tpl.calib_score > 0.7, "potrivire probe {}", tpl.calib_score);

        // Test: 20 s cu 4 cufere și 6 împușcături.
        let mut sig: Vec<f32> = (0..SR as usize * 20).map(|_| rng.f() * 0.03).collect();
        let chests = [2.0, 6.5, 11.0, 16.0];
        for &c in &chests {
            chest_sound(&mut sig, (c * SR as f32) as usize, 0.45);
        }
        for &g in &[1.0, 4.0, 8.0, 9.0, 13.5, 18.5] {
            gunshot(&mut sig, (g * SR as f32) as usize, &mut rng);
        }
        let hits = run(&mut det, &mut ex, &sig);
        assert_eq!(hits.len(), chests.len(), "detecții: {hits:?}");
    }

    #[test]
    fn needs_two_similar_samples() {
        let a = vec![0.1f32; TPL_FRAMES * BANDS];
        assert!(build_audio_template(&[a.clone()]).is_err());
        let mut r = Rng(7);
        let b: Vec<f32> = (0..TPL_FRAMES * BANDS).map(|_| r.f()).collect();
        let c: Vec<f32> = (0..TPL_FRAMES * BANDS).map(|_| r.f()).collect();
        assert!(build_audio_template(&[b, c]).is_err());
    }
}
