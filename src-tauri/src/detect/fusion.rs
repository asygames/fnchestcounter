//! Combină evenimentele vizuale și audio într-o singură numărătoare, fără dubluri.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Numără la oricare semnal (cel mai sensibil). Dublurile din aceeași deschidere se ignoră.
    #[default]
    Either,
    /// Numără doar când se potrivesc amândouă (cel mai sigur).
    Both,
    Visual,
    Audio,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Visual,
    Audio,
    Both,
    Manual,
}

/// Fereastra în care două semnale sunt considerate aceeași deschidere.
const PAIR_MS: u64 = 1800;
/// Distanța minimă între două cufere numărate.
const MIN_GAP_MS: u64 = 1000;

#[derive(Debug, Default)]
pub struct Fusion {
    pub mode: Mode,
    last_visual: Option<u64>,
    last_audio: Option<u64>,
    last_count: Option<u64>,
    /// Ultimul semnal deja „consumat” de o numărare (ca să nu-l refolosim).
    consumed_visual: Option<u64>,
    consumed_audio: Option<u64>,
}

impl Fusion {
    pub fn new(mode: Mode) -> Self {
        Self { mode, ..Default::default() }
    }

    fn gap_ok(&self, now: u64) -> bool {
        self.last_count.is_none_or(|t| now.saturating_sub(t) >= MIN_GAP_MS)
    }

    fn recent(t: Option<u64>, consumed: Option<u64>, now: u64) -> bool {
        matches!(t, Some(t) if now.saturating_sub(t) <= PAIR_MS && consumed != Some(t))
    }

    /// Primește un semnal. Întoarce sursa dacă trebuie numărat un cufăr.
    pub fn signal(&mut self, src: Source, now: u64) -> Option<Source> {
        match src {
            Source::Visual => self.last_visual = Some(now),
            Source::Audio => self.last_audio = Some(now),
            Source::Both | Source::Manual => return Some(src),
        }
        let other_recent = match src {
            Source::Visual => Self::recent(self.last_audio, self.consumed_audio, now),
            _ => Self::recent(self.last_visual, self.consumed_visual, now),
        };
        let result = match self.mode {
            Mode::Visual if src == Source::Visual => Some(Source::Visual),
            Mode::Audio if src == Source::Audio => Some(Source::Audio),
            Mode::Visual | Mode::Audio => None,
            Mode::Both => other_recent.then_some(Source::Both),
            Mode::Either => {
                // Orice semnal al celeilalte surse în fereastră = aceeași deschidere.
                let other_any = match src {
                    Source::Visual => Self::recent(self.last_audio, None, now),
                    _ => Self::recent(self.last_visual, None, now),
                };
                if other_any {
                    // Al doilea semnal pentru aceeași deschidere: deja numărat la primul.
                    self.consume(src, now);
                    return None;
                }
                Some(src)
            }
        };
        if result.is_some() {
            if !self.gap_ok(now) {
                return None;
            }
            self.last_count = Some(now);
            self.consume(src, now);
            if result == Some(Source::Both) {
                self.consumed_visual = self.last_visual;
                self.consumed_audio = self.last_audio;
            }
        }
        result
    }

    fn consume(&mut self, src: Source, now: u64) {
        match src {
            Source::Visual => self.consumed_visual = Some(now),
            Source::Audio => self.consumed_audio = Some(now),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn either_dedups_same_opening() {
        let mut f = Fusion::new(Mode::Either);
        assert_eq!(f.signal(Source::Visual, 1000), Some(Source::Visual));
        assert_eq!(f.signal(Source::Audio, 1300), None);
        assert_eq!(f.signal(Source::Audio, 5000), Some(Source::Audio));
        assert_eq!(f.signal(Source::Visual, 5200), None);
        assert_eq!(f.signal(Source::Visual, 9000), Some(Source::Visual));
        // Sunetul vine la 1,5 s după imagine (peste pragul de 1 s): tot aceeași deschidere.
        assert_eq!(f.signal(Source::Audio, 10_500), None);
    }

    #[test]
    fn both_requires_pair() {
        let mut f = Fusion::new(Mode::Both);
        assert_eq!(f.signal(Source::Visual, 1000), None);
        assert_eq!(f.signal(Source::Audio, 1500), Some(Source::Both));
        // Același semnal vizual nu poate fi folosit din nou.
        assert_eq!(f.signal(Source::Audio, 2600), None);
        assert_eq!(f.signal(Source::Audio, 9000), None);
        assert_eq!(f.signal(Source::Visual, 9500), Some(Source::Both));
    }

    #[test]
    fn single_modes_ignore_other_source() {
        let mut f = Fusion::new(Mode::Audio);
        assert_eq!(f.signal(Source::Visual, 1000), None);
        assert_eq!(f.signal(Source::Audio, 1100), Some(Source::Audio));
        assert_eq!(f.signal(Source::Audio, 1500), None, "prea repede");
    }
}
