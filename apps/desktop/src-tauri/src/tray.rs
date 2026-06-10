//! Menu-bar status item (spec 10 §3). The app is menu-bar-first: no Dock
//! icon by default; the tray glyph mirrors the supervisor state.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager};

use crate::supervisor::Status;

pub const TRAY_ID: &str = "main";

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let item = |id: &str, label: &str| MenuItem::with_id(app, id, label, true, None::<&str>);
    let start = item("start", "Start")?;
    let stop = item("stop", "Stop")?;
    let restart = item("restart", "Restart")?;
    let open_web = item("open_web", "Open web app")?;
    let reveal = item("reveal", "Reveal brain in Finder")?;
    let copy_url = item("copy_url", "Copy tailnet sync URL")?;
    let status = item("status", "Status…")?;
    let logs = item("logs", "Logs…")?;
    let settings = item("settings", "Settings…")?;
    let quit = item("quit", "Quit Contextful")?;
    let sep = || PredefinedMenuItem::separator(app);

    let menu = Menu::with_items(
        app,
        &[
            &status,
            &sep()?,
            &start,
            &stop,
            &restart,
            &sep()?,
            &open_web,
            &reveal,
            &copy_url,
            &sep()?,
            &logs,
            &settings,
            &sep()?,
            &quit,
        ],
    )?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(app.default_window_icon().cloned().expect("bundle icon"))
        .icon_as_template(true)
        .tooltip("Contextful")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| {
            let id = event.id.as_ref();
            match id {
                "start" => crate::commands::supervisor_of(app).start(),
                "stop" => crate::commands::supervisor_of(app).stop(),
                "restart" => {
                    let sup = crate::commands::supervisor_of(app);
                    tauri::async_runtime::spawn(async move { sup.restart().await });
                }
                "open_web" => crate::commands::open_web_app(),
                "reveal" => crate::commands::reveal_brain(),
                "copy_url" => {
                    let _ = crate::commands::copy_sync_url();
                }
                "status" | "logs" | "settings" => show_main(app, id),
                "quit" => {
                    tauri::async_runtime::spawn(crate::supervisor::graceful_shutdown(app.clone()));
                }
                _ => {}
            }
        })
        .build(app)?;
    Ok(())
}

/// Mirror the supervisor state in the menu bar (spec 10 §3 status item).
pub fn reflect(app: &AppHandle, status: Status) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_title(Some(status.glyph()));
        let _ = tray.set_tooltip(Some(status.tooltip()));
    }
}

pub fn show_main(app: &AppHandle, route: &str) {
    let _ = app.emit("navigate", route.to_string());
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
}
