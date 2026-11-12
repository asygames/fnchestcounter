//! Iconița din tray: arată/ascunde fereastra, overlay, sesiune nouă, ieșire.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

pub const TRAY_ID: &str = "main-tray";

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Arată fereastra", true, None::<&str>)?;
    let overlay = MenuItem::with_id(app, "overlay", "Overlay pornit / oprit", true, None::<&str>)?;
    let session = MenuItem::with_id(app, "session", "Sesiune nouă", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Ieșire", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&show, &overlay, &session, &sep, &quit])?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("FN Chest Counter")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, ev| match ev.id().as_ref() {
            "show" => show_main(app),
            "overlay" => crate::commands::toggle_overlay(app),
            "session" => crate::commands::new_session_inner(app),
            "quit" => crate::commands::quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, ev| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

pub fn update_tooltip(app: &AppHandle, session: i64, today: i64) {
    if let Some(t) = app.tray_by_id(TRAY_ID) {
        let _ = t.set_tooltip(Some(format!("FN Chest Counter: {session} în sesiune, {today} azi")));
    }
}
