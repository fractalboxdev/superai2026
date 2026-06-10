//! Contextful menu-bar shell (spec 10): a thin, trustworthy launcher around
//! the bundled `sync` binary. It adds no new authority — every brain/relay
//! call still goes through the binary's capability path.

mod commands;
mod identity;
mod keychain;
mod launchagent;
mod settings;
mod sidecar;
mod supervisor;
mod tailscale;
mod tray;
mod util;

use std::sync::Arc;

use tauri::Manager;

use crate::commands::AppCtx;
use crate::settings::AppSettings;
use crate::supervisor::Supervisor;

fn main() {
    let headless = std::env::args().any(|a| a == "--headless");

    tauri::Builder::default()
        .setup(move |app| {
            // Menu-bar-first: no Dock icon (spec 10 §3).
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let supervisor = Arc::new(Supervisor::new(app.handle().clone()));
            app.manage(AppCtx {
                supervisor: supervisor.clone(),
            });

            tray::build(app.handle())?;

            // launchctl unload / system shutdown send SIGTERM: stop the child
            // before dying so it never outlives its supervisor (spec 10 §5).
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    use tokio::signal::unix::{signal, SignalKind};
                    let (Ok(mut term), Ok(mut int)) = (
                        signal(SignalKind::terminate()),
                        signal(SignalKind::interrupt()),
                    ) else {
                        return;
                    };
                    tokio::select! {
                        _ = term.recv() => {}
                        _ = int.recv() => {}
                    }
                    supervisor::graceful_shutdown(handle).await;
                });
            }

            let settings = AppSettings::load();
            if settings.configured {
                // Configured machines come up supervised straight away —
                // launchd starts the app, the app starts the binary (§5).
                supervisor.start();
            } else if !headless {
                tray::show_main(app.handle(), "status");
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window hides it; the app lives in the menu bar.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_state,
            commands::save_settings,
            commands::mark_configured,
            commands::ensure_identity,
            commands::detect_tailscale,
            commands::start_service,
            commands::stop_service,
            commands::restart_service,
            commands::get_logs,
            commands::set_autostart,
            commands::open_web_app,
            commands::reveal_brain,
            commands::copy_sync_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Contextful");
}
