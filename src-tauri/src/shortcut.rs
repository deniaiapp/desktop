use tauri::{plugin::TauriPlugin, AppHandle, Wry};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::webview::toggle_main_window;

/// System-wide Alt+Shift+Space: show Deni AI, or hide it if it is already in front.
fn toggle_window_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::Space)
}

pub(crate) fn build_plugin() -> TauriPlugin<Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state == ShortcutState::Pressed && *shortcut == toggle_window_shortcut() {
                toggle_main_window(app);
            }
        })
        .build()
}

/// Registered from `setup` rather than the plugin builder so a combo already
/// taken by another app is logged instead of aborting startup.
pub(crate) fn register_global_shortcuts(app: &AppHandle) {
    if let Err(error) = app.global_shortcut().register(toggle_window_shortcut()) {
        eprintln!("failed to register global shortcut: {}", error);
    }
}
