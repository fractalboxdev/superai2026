//! Menu-bar status item (spec 10 §3). The app is menu-bar-first: no Dock
//! icon by default; the tray glyph mirrors the supervisor state.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager};

use crate::supervisor::Status;

pub const TRAY_ID: &str = "main";

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let start = MenuItem::with_id(app, "start", "Start", true, None::<&str>)?;
    let stop = MenuItem::with_id(app, "stop", "Stop", true, None::<&str>)?;
    let restart = MenuItem::with_id(app, "restart", "Restart", true, None::<&str>)?;
    let open_web = MenuItem::with_id(app, "open_web", "Open web app", true, None::<&str>)?;
    let reveal = MenuItem::with_id(app, "reveal", "Reveal brain in Finder", true, None::<&str>)?;
    let copy_url = MenuItem::with_id(app, "copy_url", "Copy tailnet sync URL", true, None::<&str>)?;
    let status = MenuItem::with_id(app, "status", "Status…", true, None::<&str>)?;
    let logs = MenuItem::with_id(app, "logs", "Logs…", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Contextful", true, None::<&str>)?;
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
                "open_web" => crate::commands::open_web(app),
                "reveal" => crate::commands::reveal_brain_dir(app),
                "copy_url" => {
                    let _ = crate::commands::copy_sync_url_impl(app);
                }
                "status" | "logs" | "settings" => show_main(app, id),
                "quit" => {
                    let sup = crate::commands::supervisor_of(app);
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        sup.stop();
                        // brief drain so the child is reaped before we exit
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        app.exit(0);
                    });
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
        let title = match status {
            Status::Healthy => "",
            Status::Starting => "…",
            Status::Degraded => "!",
            Status::Stopped => "·",
        };
        let _ = tray.set_title(Some(title));
        let tip = match status {
            Status::Healthy => "Contextful — running",
            Status::Starting => "Contextful — starting",
            Status::Degraded => "Contextful — running, with issues",
            Status::Stopped => "Contextful — stopped",
        };
        let _ = tray.set_tooltip(Some(tip));
    }
}

pub fn show_main(app: &AppHandle, route: &str) {
    let _ = app.emit("navigate", route.to_string());
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
}
