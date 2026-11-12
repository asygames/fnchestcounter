#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::start;

/// Pe alte sisteme aplicația pornește (pentru dezvoltarea interfeței), dar fără detecție.
#[cfg(not(windows))]
pub fn start(engine: std::sync::Arc<crate::engine::Engine>) {
    let mut l = engine.live.lock();
    l.capture = "unsupported".into();
    l.audio = "unsupported".into();
}

pub const SUPPORTED: bool = cfg!(windows);
