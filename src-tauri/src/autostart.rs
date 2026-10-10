use tauri::{plugin::TauriPlugin, AppHandle, Wry};
use tauri_plugin_autostart::ManagerExt;

use crate::state::app_title;
use crate::webview::notify;

/// Passed by the OS login item so the app knows to start quietly in the tray.
pub(crate) const AUTOSTART_ARG: &str = "--autostart";

pub(crate) fn build_plugin() -> TauriPlugin<Wry> {
    tauri_plugin_autostart::Builder::new()
        .arg(AUTOSTART_ARG)
        .build()
}

pub(crate) fn launched_by_autostart() -> bool {
    std::env::args().any(|arg| arg == AUTOSTART_ARG)
}

pub(crate) fn is_autostart_enabled(app: &AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

/// Flips launch-at-login and returns the state the OS actually ended up in.
pub(crate) fn toggle_autostart(app: &AppHandle) -> bool {
    let manager = app.autolaunch();
    let result = if is_autostart_enabled(app) {
        manager.disable()
    } else {
        manager.enable()
    };

    if let Err(error) = result {
        notify(
            app,
            app_title(app),
            format!("Couldn't change Launch at Login: {error}"),
        );
    }

    is_autostart_enabled(app)
}
