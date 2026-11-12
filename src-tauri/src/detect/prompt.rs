//! Detecția automată a promptului de cufăr („E  SEARCH”), fără calibrare.
//!
//! În Fortnite-ul actual promptul nu mai stă într-un loc fix: e lipit de cufăr și se
//! mută pe ecran odată cu el. De aceea nu mai căutăm un șablon într-o zonă fixă, ci:
//! 1. `find_candidates`: în toată zona centrală a ecranului căutăm rânduri de text alb
//!    pe un panou întunecat (exact cum arată promptul). Durează câteva milisecunde.
//! 2. `ocr_crop`: decupăm fiecare candidat, îl mărim și îl dăm OCR-ului din Windows.
//! 3. `match_words`: căutăm în textul citit cuvântul promptului („SEARCH”).
//! 4. `gold_fraction`: lângă prompt trebuie să fie un cufăr auriu. Lăzile de muniție
//!    au tot „SEARCH”, dar nu sunt aurii.
//! 5. `OcrTracker`: numără un cufăr când promptul dispare după ce ai apăsat tasta.

use crate::detect::visual::Roi;

/// Zona în care căutăm promptul (fracțiuni din cadru): aproape tot ecranul,
/// fără marginile cu HUD (hartă, inventar, viață).
pub const SEARCH_REGION: Roi = Roi { x0: 0.15, y0: 0.12, x1: 0.85, y1: 0.92 };
/// Înălțimea maximă de lucru a cadrului (4K se micșorează la 1440p, restul rămân 1:1).
pub const MAX_FRAME_H: u32 = 1440;
/// Câți pixeli aurii (din zona din jurul promptului) înseamnă „e un cufăr”.
pub const GOLD_MIN: f32 = 0.06;

/// Imagine RGB, 3 octeți pe pixel.
#[derive(Clone, Debug, Default)]
pub struct Rgb {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u8>,
}

impl Rgb {
    #[inline]
    fn rgb(&self, x: usize, y: usize) -> (u8, u8, u8) {
        let i = (y * self.w + x) * 3;
        (self.px[i], self.px[i + 1], self.px[i + 2])
    }
}

/// Dreptunghi în pixeli.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoxI {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct Candidate {
    /// Cutia textului alb.
    pub b: BoxI,
    /// Distanța față de centrul zonei (pentru ordine).
    pub dist: f32,
}

/// Imagine gri pregătită pentru OCR (text închis pe fundal deschis).
#[derive(Clone, Debug)]
pub struct OcrImage {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u8>,
}

/// Micșorează un buffer BGRA/RGBA (cu row pitch) la `out_w × out_h` RGB, prin medie pe blocuri.
pub fn downscale_rgb(buf: &[u8], w: usize, h: usize, pitch: usize, bgra: bool, out_w: usize, out_h: usize) -> Rgb {
    let mut out = Rgb { w: out_w, h: out_h, px: vec![0; out_w * out_h * 3] };
    if w == 0 || h == 0 || out_w == 0 || out_h == 0 {
        return out;
    }
    let (ri, bi) = if bgra { (2, 0) } else { (0, 2) };
    for oy in 0..out_h {
        let sy0 = oy * h / out_h;
        let sy1 = ((oy + 1) * h / out_h).max(sy0 + 1).min(h);
        for ox in 0..out_w {
            let sx0 = ox * w / out_w;
            let sx1 = ((ox + 1) * w / out_w).max(sx0 + 1).min(w);
            let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
            for y in sy0..sy1 {
                let row = y * pitch;
                for x in sx0..sx1 {
                    let i = row + x * 4;
                    if i + 2 < buf.len() {
                        r += buf[i + ri] as u32;
                        g += buf[i + 1] as u32;
                        b += buf[i + bi] as u32;
                        n += 1;
                    }
                }
            }
            if let (Some(r), Some(g), Some(b)) = (r.checked_div(n), g.checked_div(n), b.checked_div(n)) {
                let o = (oy * out_w + ox) * 3;
                out.px[o] = r as u8;
                out.px[o + 1] = g as u8;
                out.px[o + 2] = b as u8;
            }
        }
    }
    out
}

const F_WHITE: u8 = 1;
const F_DARK: u8 = 2;
const F_TEXT: u8 = 4;
const F_SEEN: u8 = 8;

/// Caută rânduri de text alb pe panou întunecat. Întoarce cel mult 6 candidați,
/// cei mai apropiați de centru primii.
pub fn find_candidates(img: &Rgb) -> Vec<Candidate> {
    let (w, h) = (img.w, img.h);
    if w < 16 || h < 16 || img.px.len() < w * h * 3 {
        return Vec::new();
    }
    let mut f = vec![0u8; w * h];
    for (i, fl) in f.iter_mut().enumerate() {
        let (r, g, b) = (img.px[i * 3] as i32, img.px[i * 3 + 1] as i32, img.px[i * 3 + 2] as i32);
        let l = (77 * r + 150 * g + 29 * b) >> 8;
        let sat = r.max(g).max(b) - r.min(g).min(b);
        if l >= 170 && sat <= 70 {
            *fl = F_WHITE;
        } else if l <= 110 {
            *fl = F_DARK;
        }
    }
    // Pixel de text = alb cu un pixel întunecat aproape (sus/jos/stânga/dreapta).
    const R: usize = 4;
    let mut text_count = 0usize;
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if f[i] & F_WHITE == 0 {
                continue;
            }
            let mut near_dark = false;
            for d in 1..=R {
                if (x >= d && f[i - d] & F_DARK != 0)
                    || (x + d < w && f[i + d] & F_DARK != 0)
                    || (y >= d && f[i - d * w] & F_DARK != 0)
                    || (y + d < h && f[i + d * w] & F_DARK != 0)
                {
                    near_dark = true;
                    break;
                }
            }
            if near_dark {
                f[i] |= F_TEXT;
                text_count += 1;
            }
        }
    }
    if text_count < 12 {
        return Vec::new();
    }
    // Unim literele unui cuvânt: pixeli de text în același rând, la cel mult GAP unul de altul,
    // și rânduri vecine. Facem asta cu o căutare pe componente, cu vecinătate lărgită pe orizontală.
    const GAP: i32 = 6;
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let mut out = Vec::new();
    let mut stack: Vec<(i32, i32)> = Vec::new();
    for sy in 0..h {
        for sx in 0..w {
            let si = sy * w + sx;
            if f[si] & F_TEXT == 0 || f[si] & F_SEEN != 0 {
                continue;
            }
            f[si] |= F_SEEN;
            stack.clear();
            stack.push((sx as i32, sy as i32));
            let (mut x0, mut y0, mut x1, mut y1) = (sx as i32, sy as i32, sx as i32, sy as i32);
            let mut count = 0u32;
            while let Some((x, y)) = stack.pop() {
                count += 1;
                x0 = x0.min(x);
                x1 = x1.max(x);
                y0 = y0.min(y);
                y1 = y1.max(y);
                for dy in -1..=1 {
                    let ny = y + dy;
                    if ny < 0 || ny >= h as i32 {
                        continue;
                    }
                    for dx in -GAP..=GAP {
                        let nx = x + dx;
                        if nx < 0 || nx >= w as i32 {
                            continue;
                        }
                        let ni = ny as usize * w + nx as usize;
                        if f[ni] & F_TEXT != 0 && f[ni] & F_SEEN == 0 {
                            f[ni] |= F_SEEN;
                            stack.push((nx, ny));
                        }
                    }
                }
                if count > 40_000 {
                    break; // zonă uriașă (zăpadă, cer): nu e un prompt
                }
            }
            let (bw, bh) = (x1 - x0 + 1, y1 - y0 + 1);
            if !(6..=48).contains(&bh) || bw < bh * 2 || bw > bh * 40 {
                continue;
            }
            let density = count as f32 / (bw * bh) as f32;
            if !(0.10..=0.80).contains(&density) {
                continue;
            }
            // Panoul: în jurul textului, majoritatea pixelilor care nu sunt text trebuie să fie întunecați.
            let m = (bh / 2).max(2);
            let (ex0, ey0) = ((x0 - m).max(0), (y0 - m).max(0));
            let (ex1, ey1) = ((x1 + m).min(w as i32 - 1), (y1 + m).min(h as i32 - 1));
            let (mut dark, mut other) = (0u32, 0u32);
            for y in ey0..=ey1 {
                for x in ex0..=ex1 {
                    let v = f[y as usize * w + x as usize];
                    if v & F_TEXT != 0 {
                        continue;
                    }
                    if v & F_DARK != 0 {
                        dark += 1;
                    } else {
                        other += 1;
                    }
                }
            }
            if dark * 100 < (dark + other) * 35 {
                continue;
            }
            let (mx, my) = (x0 as f32 + bw as f32 / 2.0, y0 as f32 + bh as f32 / 2.0);
            let dist = ((mx - cx) / w as f32).hypot((my - cy) / h as f32);
            out.push(Candidate { b: BoxI { x: x0, y: y0, w: bw, h: bh }, dist });
        }
    }
    out.sort_by(|a, b| a.dist.total_cmp(&b.dist));
    out.truncate(6);
    out
}

/// Decupează candidatul cu margine, îl mărește (textul ajunge la ~32 px) și îl inversează
/// (text negru pe alb), cum îi place OCR-ului.
pub fn ocr_crop(img: &Rgb, b: BoxI) -> OcrImage {
    let pad = b.h.max(4);
    let (x0, y0) = ((b.x - pad).max(0), (b.y - pad).max(0));
    let (x1, y1) = ((b.x + b.w + pad).min(img.w as i32), (b.y + b.h + pad).min(img.h as i32));
    let (cw, ch) = ((x1 - x0).max(1) as usize, (y1 - y0).max(1) as usize);
    let scale = (32.0 / b.h.max(1) as f32).clamp(1.0, 5.0);
    let (ow, oh) = (((cw as f32 * scale) as usize).max(1), ((ch as f32 * scale) as usize).max(1));
    // „Albeața”: luminozitate minus saturație, ca auriul strălucitor să nu pară text.
    let white = |x: usize, y: usize| -> f32 {
        let (r, g, b) = img.rgb(x, y);
        let (r, g, b) = (r as f32, g as f32, b as f32);
        let l = 0.299 * r + 0.587 * g + 0.114 * b;
        l - 1.5 * (r.max(g).max(b) - r.min(g).min(b))
    };
    let (gx, gy) = (x0 as usize, y0 as usize);
    let mut px = vec![255u8; ow * oh];
    for oy in 0..oh {
        let fy = ((oy as f32 + 0.5) / scale - 0.5).clamp(0.0, (ch - 1) as f32);
        let (iy, ty) = (fy.floor() as usize, fy.fract());
        let iy1 = (iy + 1).min(ch - 1);
        for ox in 0..ow {
            let fx = ((ox as f32 + 0.5) / scale - 0.5).clamp(0.0, (cw - 1) as f32);
            let (ix, tx) = (fx.floor() as usize, fx.fract());
            let ix1 = (ix + 1).min(cw - 1);
            let a = white(gx + ix, gy + iy) * (1.0 - tx) + white(gx + ix1, gy + iy) * tx;
            let c = white(gx + ix, gy + iy1) * (1.0 - tx) + white(gx + ix1, gy + iy1) * tx;
            let v = a * (1.0 - ty) + c * ty;
            // Prag moale: text alb (≥ 200) → negru, fundal (≤ 120) → alb, margini netede între ele.
            let t = ((v - 120.0) / 80.0).clamp(0.0, 1.0);
            px[oy * ow + ox] = (255.0 * (1.0 - t)) as u8;
        }
    }
    OcrImage { w: ow, h: oh, px }
}

/// Zona din jurul centrului ecranului (în coordonatele zonei de căutare) pe care o citim
/// direct cu OCR când ai apăsat tasta și căutarea rapidă n-a găsit promptul.
pub fn center_area(img: &Rgb) -> BoxI {
    let (w, h) = (img.w as f32, img.h as f32);
    // Centrul ecranului e la ~(0,50; 0,475) din zona de căutare; promptul stă de obicei
    // lângă cufărul din fața ta, deci luăm o zonă largă în jurul lui.
    let (x0, x1) = ((w * 0.20) as i32, (w * 0.86) as i32);
    let (y0, y1) = ((h * 0.18) as i32, (h * 0.84) as i32);
    BoxI { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
}

/// Pregătește o zonă mare pentru OCR: text alb → negru, restul alb, mărită ca textul
/// promptului să aibă ~20–30 px. Întoarce imaginea și factorul de mărire.
pub fn ocr_area(img: &Rgb, a: BoxI) -> (OcrImage, f32) {
    // Promptul are ~1,4% din înălțimea cadrului; zona de căutare e 80% din cadru.
    let frame_h = img.h as f32 / 0.80;
    let text_h = (frame_h * 0.014).max(4.0);
    let scale = (24.0 / text_h).clamp(1.0, 3.0);
    let x0 = a.x.max(0) as usize;
    let y0 = a.y.max(0) as usize;
    let x1 = ((a.x + a.w) as usize).min(img.w);
    let y1 = ((a.y + a.h) as usize).min(img.h);
    let (cw, ch) = (x1.saturating_sub(x0).max(1), y1.saturating_sub(y0).max(1));
    let (ow, oh) = (((cw as f32 * scale) as usize).max(1), ((ch as f32 * scale) as usize).max(1));
    let mut px = vec![255u8; ow * oh];
    for oy in 0..oh {
        let sy = (y0 + ((oy as f32 / scale) as usize).min(ch - 1)).min(img.h - 1);
        for ox in 0..ow {
            let sx = (x0 + ((ox as f32 / scale) as usize).min(cw - 1)).min(img.w - 1);
            let (r, g, b) = img.rgb(sx, sy);
            let (r, g, b) = (r as f32, g as f32, b as f32);
            let v = 0.299 * r + 0.587 * g + 0.114 * b - 1.5 * (r.max(g).max(b) - r.min(g).min(b));
            let t = ((v - 120.0) / 80.0).clamp(0.0, 1.0);
            px[oy * ow + ox] = (255.0 * (1.0 - t)) as u8;
        }
    }
    (OcrImage { w: ow, h: oh, px }, scale)
}

/// Ce parte din zona din jurul promptului e aurie (cufărul stă în stânga/sub prompt).
pub fn gold_fraction(img: &Rgb, b: BoxI) -> f32 {
    let h = b.h.max(1);
    let x0 = (b.x - 22 * h).max(0);
    let x1 = (b.x + b.w + 4 * h).min(img.w as i32);
    let y0 = (b.y - 10 * h).max(0);
    let y1 = (b.y + b.h + 13 * h).min(img.h as i32);
    if x1 <= x0 || y1 <= y0 {
        return 0.0;
    }
    let (mut gold, mut n) = (0u32, 0u32);
    let mut y = y0;
    while y < y1 {
        let mut x = x0;
        while x < x1 {
            let (r, g, bl) = img.rgb(x as usize, y as usize);
            n += 1;
            if is_gold(r, g, bl) {
                gold += 1;
            }
            x += 2;
        }
        y += 2;
    }
    gold as f32 / n.max(1) as f32
}

/// Auriu/galben: nuanță 25–62°, saturație > 0,45, luminozitate > 0,35.
#[inline]
pub fn is_gold(r: u8, g: u8, b: u8) -> bool {
    let (r, g, b) = (r as f32, g as f32, b as f32);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    if max < 0.35 * 255.0 || max - min < 0.45 * max {
        return false;
    }
    // Nuanța, doar pentru cazul în care roșul sau verdele e maxim (auriul nu are albastru dominant).
    let d = max - min;
    let hue = if max == r {
        60.0 * ((g - b) / d)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        return false;
    };
    (25.0..=62.0).contains(&hue)
}

/// Normalizează un cuvânt citit: majuscule, fără semne, cu confuziile tipice de OCR reparate.
fn normalize(word: &str) -> String {
    word.chars()
        .filter_map(|c| {
            let c = match c {
                '5' => 'S',
                '0' => 'O',
                '1' | '|' => 'I',
                '8' => 'B',
                c => c,
            };
            c.is_alphanumeric().then(|| c.to_uppercase().next().unwrap_or(c))
        })
        .collect()
}

fn levenshtein(a: &[char], b: &[char]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + usize::from(ca != cb));
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Lista de cuvinte din setări („SEARCH, FOUILLER”) → cuvinte normalizate.
pub fn parse_words(s: &str) -> Vec<String> {
    s.split([',', ';', '\n'])
        .map(normalize)
        .filter(|w| w.chars().count() >= 3)
        .collect()
}

/// Întoarce cuvântul găsit dacă textul citit conține unul din `words` (cu toleranță la o literă greșită).
pub fn match_words(text: &str, words: &[String]) -> Option<String> {
    for raw in text.split_whitespace() {
        let tok = normalize(raw);
        if tok.len() < 3 {
            continue;
        }
        let tc: Vec<char> = tok.chars().collect();
        for kw in words {
            let kc: Vec<char> = kw.chars().collect();
            let tol = if kc.len() >= 5 { 1 } else { 0 };
            // „ESEARCH” (tasta lipită de cuvânt) sau „SEARCHING” se acceptă și ele.
            let contains = tc.len() <= kc.len() + 3 && tok.contains(kw.as_str());
            if contains || levenshtein(&tc, &kc) <= tol {
                return Some(tok);
            }
        }
    }
    None
}

/// Starea promptului citit cu OCR și decizia „cufăr deschis”.
#[derive(Debug, Default)]
pub struct OcrTracker {
    visible: bool,
    misses: u32,
    since_ms: u64,
    gold_max: f32,
}

/// Fereastra în care apăsarea tastei contează pentru deschiderea cufărului.
pub const KEY_WINDOW_MS: u64 = 2500;

impl OcrTracker {
    pub const fn new() -> Self {
        Self { visible: false, misses: 0, since_ms: 0, gold_max: 0.0 }
    }

    pub fn visible(&self) -> bool {
        self.visible
    }

    pub fn gold_max(&self) -> f32 {
        self.gold_max
    }

    /// Un rezultat nou. `found` = promptul („SEARCH”) e pe ecran, `gold` = cât auriu e lângă el.
    /// `last_key_ms` = ultima dată când tasta de interacțiune era apăsată.
    /// Întoarce `true` dacă tocmai s-a deschis un cufăr.
    pub fn update(&mut self, found: bool, gold: f32, now_ms: u64, last_key_ms: Option<u64>, require_key: bool, chest_only: bool) -> bool {
        if found {
            if !self.visible {
                self.visible = true;
                self.since_ms = now_ms;
                self.gold_max = gold;
            } else {
                self.gold_max = self.gold_max.max(gold);
            }
            self.misses = 0;
            return false;
        }
        if !self.visible {
            return false;
        }
        self.misses += 1;
        if self.misses < 2 {
            return false;
        }
        self.visible = false;
        self.misses = 0;
        let chest_ok = !chest_only || self.gold_max >= GOLD_MIN;
        let key_ok = if require_key {
            // Tasta trebuie apăsată cât timp se vedea promptul (nu înainte, la altceva).
            last_key_ms.is_some_and(|k| k + 250 >= self.since_ms && now_ms.saturating_sub(k) <= KEY_WINDOW_MS)
        } else {
            (150..20_000).contains(&now_ms.saturating_sub(self.since_ms))
        };
        chest_ok && key_ok
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Captura reală a promptului din joc (cufăr auriu, „E SEARCH”), 421×362.
    pub(crate) const SHOT: &[u8] = include_bytes!("../../tests/fixtures/chest_search.png");
    /// Cutia cuvântului „SEARCH” în captură (măsurată de mână).
    const WORD: BoxI = BoxI { x: 259, y: 207, w: 53, h: 9 };
    /// A doua captură reală: cufăr lângă un perete galben, 518×480.
    pub(crate) const SHOT2: &[u8] = include_bytes!("../../tests/fixtures/chest_search2.png");
    const WORD2: BoxI = BoxI { x: 355, y: 244, w: 52, h: 10 };

    pub(crate) fn load_png(bytes: &[u8]) -> Rgb {
        let dec = png::Decoder::new(std::io::Cursor::new(bytes));
        let mut r = dec.read_info().expect("png");
        let mut buf = vec![0; r.output_buffer_size().expect("mărime png")];
        let info = r.next_frame(&mut buf).expect("cadru png");
        let ch = match info.color_type {
            png::ColorType::Rgba => 4,
            png::ColorType::Rgb => 3,
            other => panic!("format neașteptat {other:?}"),
        };
        let (w, h) = (info.width as usize, info.height as usize);
        let mut px = Vec::with_capacity(w * h * 3);
        for p in buf[..w * h * ch].chunks(ch) {
            px.extend_from_slice(&p[..3]);
        }
        Rgb { w, h, px }
    }

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u8 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 24) as u8
        }
    }

    /// Cadru mare de „joc” (zgomot colorat cu pete) cu captura lipită la (ox, oy).
    fn scene(shot: &Rgb, w: usize, h: usize, ox: usize, oy: usize, seed: u64) -> Rgb {
        let mut r = Rng(seed * 2654435761 + 1);
        let mut img = Rgb { w, h, px: vec![0; w * h * 3] };
        let base = [r.next() / 2 + 40, r.next() / 2 + 60, r.next() / 2 + 30];
        for y in 0..h {
            for x in 0..w {
                let o = (y * w + x) * 3;
                let wave = ((x as f32 / 37.0).sin() * 30.0 + (y as f32 / 23.0).cos() * 25.0) as i32;
                for (c, &bc) in base.iter().enumerate() {
                    img.px[o + c] = (bc as i32 + wave + (r.next() % 24) as i32).clamp(0, 255) as u8;
                }
            }
        }
        for y in 0..shot.h {
            for x in 0..shot.w {
                let (s, d) = ((y * shot.w + x) * 3, ((y + oy) * w + x + ox) * 3);
                img.px[d..d + 3].copy_from_slice(&shot.px[s..s + 3]);
            }
        }
        img
    }

    pub(crate) fn scaled(img: &Rgb, f: f32) -> Rgb {
        let (w, h) = ((img.w as f32 * f) as usize, (img.h as f32 * f) as usize);
        let mut out = Rgb { w, h, px: vec![0; w * h * 3] };
        for y in 0..h {
            for x in 0..w {
                let (sx, sy) = (((x as f32 / f) as usize).min(img.w - 1), ((y as f32 / f) as usize).min(img.h - 1));
                let (s, d) = ((sy * img.w + sx) * 3, (y * w + x) * 3);
                out.px[d..d + 3].copy_from_slice(&img.px[s..s + 3]);
            }
        }
        out
    }

    fn overlaps(a: BoxI, b: BoxI) -> bool {
        a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
    }

    #[test]
    fn finds_search_word_in_real_screenshots() {
        for (shot, word) in [(SHOT, WORD), (SHOT2, WORD2)] {
            let shot = load_png(shot);
            let c = find_candidates(&shot);
            let hit = c.iter().find(|c| overlaps(c.b, word) && (c.b.h - word.h).abs() <= 3);
            let hit = hit.unwrap_or_else(|| panic!("candidați: {c:?}"));
            let gold = gold_fraction(&shot, hit.b);
            eprintln!("{}×{}: cutie {:?}, auriu {:.0}%", shot.w, shot.h, hit.b, gold * 100.0);
            assert!(gold >= GOLD_MIN * 2.0, "auriu {gold}");
        }
    }

    /// Un cadru întreg din joc (1271×726, cu HUD, kill feed, nume de coechipieri):
    /// decupăm zona de căutare ca în aplicație, iar promptul trebuie să fie printre
    /// primii 4 candidați (doar pe ei îi citim cu OCR).
    #[test]
    fn finds_prompt_in_real_full_frame() {
        let full = load_png(include_bytes!("../../tests/fixtures/frame_full.png"));
        let (x0, y0, x1, y1) = SEARCH_REGION.pixels(full.w as u32, full.h as u32);
        let (x0, y0, x1, y1) = (x0 as usize, y0 as usize, x1 as usize, y1 as usize);
        let mut region = Rgb { w: x1 - x0, h: y1 - y0, px: Vec::with_capacity((x1 - x0) * (y1 - y0) * 3) };
        for y in y0..y1 {
            region.px.extend_from_slice(&full.px[(y * full.w + x0) * 3..(y * full.w + x1) * 3]);
        }
        let want = BoxI { x: 720 - x0 as i32, y: 340 - y0 as i32, w: 52, h: 10 };
        let c = find_candidates(&region);
        let pos = c.iter().take(4).position(|c| overlaps(c.b, want));
        assert!(pos.is_some(), "promptul nu e în primii 4: {c:?}");
        let b = c[pos.unwrap()].b;
        let area = center_area(&region);
        assert!(
            area.x <= want.x && area.y <= want.y && area.x + area.w >= want.x + want.w && area.y + area.h >= want.y + want.h,
            "zona centrală {area:?} nu conține promptul {want:?}"
        );
        let (oimg, scale) = ocr_area(&region, area);
        assert!(oimg.w > area.w as usize && scale >= 1.0);
        let gold = gold_fraction(&region, b);
        eprintln!("cadru întreg: {} candidați, promptul pe locul {}, auriu {:.0}%", c.len(), pos.unwrap() + 1, gold * 100.0);
        assert!(gold >= GOLD_MIN * 2.0, "auriu {gold}");
    }

    #[test]
    fn finds_prompt_anywhere_in_a_full_frame() {
        let shot = load_png(SHOT);
        // Promptul lipit în locuri diferite dintr-un cadru 1440p (zona de căutare ~1792×1152).
        for (k, &(ox, oy)) in [(40usize, 30usize), (700, 400), (1300, 760), (120, 780)].iter().enumerate() {
            let img = scene(&shot, 1792, 1152, ox, oy, k as u64);
            let want = BoxI { x: WORD.x + ox as i32, y: WORD.y + oy as i32, ..WORD };
            let t0 = std::time::Instant::now();
            let c = find_candidates(&img);
            eprintln!("find_candidates 1792×1152: {:.1} ms, {} candidați", t0.elapsed().as_secs_f32() * 1000.0, c.len());
            assert!(c.iter().any(|c| overlaps(c.b, want)), "poziția ({ox},{oy}): {c:?}");
            let best = c.iter().find(|c| overlaps(c.b, want)).unwrap();
            assert!(gold_fraction(&img, best.b) >= GOLD_MIN * 2.0, "auriu {}", gold_fraction(&img, best.b));
        }
    }

    #[test]
    fn finds_prompt_at_other_scales() {
        let shot = load_png(SHOT);
        for f in [1.5f32, 2.0, 3.0] {
            let big = scaled(&shot, f);
            let want = BoxI {
                x: (WORD.x as f32 * f) as i32,
                y: (WORD.y as f32 * f) as i32,
                w: (WORD.w as f32 * f) as i32,
                h: (WORD.h as f32 * f) as i32,
            };
            let c = find_candidates(&big);
            assert!(c.iter().any(|c| overlaps(c.b, want)), "scara {f}: {c:?}");
        }
    }

    #[test]
    fn no_candidates_on_plain_scene() {
        let empty = Rgb { w: 32, h: 32, px: vec![0; 32 * 32 * 3] };
        for s in 0..5 {
            let img = scene(&empty, 1280, 720, 0, 0, s + 10);
            assert!(find_candidates(&img).is_empty());
        }
    }

    #[test]
    fn gold_tells_chest_from_other_containers() {
        let shot = load_png(SHOT);
        assert!(gold_fraction(&shot, WORD) > 0.2, "cufăr: {}", gold_fraction(&shot, WORD));
        // Aceeași scenă desaturată (ca o ladă gri de muniție): fără auriu.
        let mut grey = shot.clone();
        for p in grey.px.chunks_mut(3) {
            let l = ((p[0] as u32 * 77 + p[1] as u32 * 150 + p[2] as u32 * 29) >> 8) as u8;
            p.copy_from_slice(&[l, l, l]);
        }
        assert!(gold_fraction(&grey, WORD) < GOLD_MIN, "gri: {}", gold_fraction(&grey, WORD));
        assert!(is_gold(230, 180, 40) && is_gold(200, 160, 30));
        assert!(!is_gold(90, 160, 60) && !is_gold(200, 200, 200) && !is_gold(37, 29, 9));
    }

    #[test]
    fn ocr_crop_is_dark_text_on_light() {
        let shot = load_png(SHOT);
        let o = ocr_crop(&shot, WORD);
        assert!(o.h >= 32 * 2 && o.w > o.h, "{}×{}", o.w, o.h);
        // Fundalul (colțurile) e deschis, iar textul (mijlocul rândului) are pixeli închiși.
        assert!(o.px[0] > 150 && o.px[o.w - 1] > 150);
        let mid = o.h / 2;
        assert!(o.px[mid * o.w..(mid + 1) * o.w].iter().any(|&v| v < 60));
        if let Ok(path) = std::env::var("FNCC_DUMP_OCR") {
            let mut f = format!("P5\n{} {}\n255\n", o.w, o.h).into_bytes();
            f.extend_from_slice(&o.px);
            std::fs::write(path, f).unwrap();
        }
    }

    #[test]
    fn word_matching_is_tolerant() {
        let w = parse_words("Search, fouiller");
        assert_eq!(w, vec!["SEARCH".to_string(), "FOUILLER".to_string()]);
        for t in ["SEARCH", "Search", "E SEARCH", "SEARCN", "5EARCH", "ESEARCH", "SEARCH."] {
            assert!(match_words(t, &w).is_some(), "{t}");
        }
        for t in ["OPEN", "PICK UP", "SEAT", "RESEARCHERS", "E", ""] {
            assert!(match_words(t, &w).is_none(), "{t}");
        }
    }

    #[test]
    fn tracker_counts_chest_after_key() {
        let mut t = OcrTracker::default();
        // Prompt la cufăr, ții E, cufărul se deschide (promptul dispare): +1.
        assert!(!t.update(true, 0.3, 1000, None, true, true));
        assert!(!t.update(true, 0.3, 1200, Some(1150), true, true));
        assert!(!t.update(false, 0.0, 1500, Some(1400), true, true));
        assert!(t.update(false, 0.0, 1600, Some(1400), true, true));
        // Treci pe lângă un cufăr fără să apeși: 0.
        assert!(!t.update(true, 0.3, 5000, Some(1400), true, true));
        assert!(!t.update(false, 0.0, 5400, Some(1400), true, true));
        assert!(!t.update(false, 0.0, 5500, Some(1400), true, true));
        // Ladă de muniție (fără auriu), cu tasta: 0.
        assert!(!t.update(true, 0.01, 9000, Some(9000), true, true));
        assert!(!t.update(false, 0.0, 9300, Some(9200), true, true));
        assert!(!t.update(false, 0.0, 9400, Some(9200), true, true));
        // Un singur cadru ratat de OCR nu închide promptul.
        assert!(!t.update(true, 0.3, 12_000, None, true, true));
        assert!(!t.update(false, 0.0, 12_100, None, true, true));
        assert!(!t.update(true, 0.3, 12_200, None, true, true));
        assert!(t.visible());
    }
}
