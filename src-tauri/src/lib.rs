pub mod commands;
pub mod detect;
pub mod engine;
pub mod platform;
pub mod store;
pub mod tray;

use engine::Engine;
use std::sync::Arc;
use std::time::Duration;
use tauri::{Emitter, Manager, WindowEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main(app);
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::get_live,
            commands::add_manual,
            commands::new_session,
            commands::reset_all,
            commands::update_settings,
            commands::visual_take_sample,
            commands::visual_clear,
            commands::audio_take_sample,
            commands::audio_clear,
            commands::visual_preview,
            commands::get_thumbnail,
            commands::overlay_edit,
            commands::simulate_chest,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let dir = app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir().join("FNChestCounter"));
            let store = store::Store::open(dir);
            let settings = store.settings.clone();
            let (session, today) = (store.session.count, store.today());
            let engine = Engine::new(handle.clone(), store);
            app.manage(engine.clone());

            tray::build(&handle)?;
            tray::update_tooltip(&handle, session, today);
            commands::register_hotkeys(&handle, &settings);
            commands::apply_overlay(&handle, &settings);
            platform::start(engine.clone());

            // Starea detectorilor spre interfață, de 5 ori pe secundă (mesaj mic).
            let e = engine.clone();
            std::thread::Builder::new()
                .name("live".into())
                .spawn(move || {
                    while !e.shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                        let live = e.live.lock().clone();
                        let _ = e.app.emit("live", &live);
                        std::thread::sleep(Duration::from_millis(200));
                    }
                })
                .ok();
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let to_tray = app.state::<Arc<Engine>>().store.lock().settings.close_to_tray;
                if to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    commands::quit(app);
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("eroare la pornirea aplicației");
}
