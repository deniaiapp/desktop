use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Wry,
};

use crate::autostart::is_autostart_enabled;
use crate::menu::{
    MENU_CHECK_UPDATES, MENU_NEW_CHAT, MENU_OPEN_DOWNLOADS, MENU_OPEN_IN_BROWSER,
    MENU_OPEN_LATEST_DOWNLOAD, MENU_QUIT, MENU_RELOAD, MENU_SHOW_WINDOW, MENU_TOGGLE_AUTOSTART,
};
use crate::state::app_title;
use crate::webview::show_main_window_from_app;

pub(crate) const TRAY_ID: &str = "main-tray";

/// Keeps the tray icon alive for the lifetime of the app; Tauri drops it
/// (and removes it from the system tray) once this state is dropped.
pub(crate) struct TrayState {
    _tray: TrayIcon<Wry>,
    autostart_item: CheckMenuItem<Wry>,
}

pub(crate) fn sync_autostart_item(app: &AppHandle, enabled: bool) {
    if let Err(error) = app.state::<TrayState>().autostart_item.set_checked(enabled) {
        eprintln!("failed to update Launch at Login item: {}", error);
    }
}

fn build_tray_menu(
    app: &AppHandle,
    autostart_item: &CheckMenuItem<Wry>,
) -> tauri::Result<Menu<Wry>> {
    let show_window = MenuItem::with_id(app, MENU_SHOW_WINDOW, "Show Deni AI", true, None::<&str>)?;
    let new_chat = MenuItem::with_id(app, MENU_NEW_CHAT, "New Chat", true, None::<&str>)?;
    let reload = MenuItem::with_id(app, MENU_RELOAD, "Reload", true, None::<&str>)?;
    let open_in_browser = MenuItem::with_id(
        app,
        MENU_OPEN_IN_BROWSER,
        "Open in Browser",
        true,
        None::<&str>,
    )?;
    let open_downloads = MenuItem::with_id(
        app,
        MENU_OPEN_DOWNLOADS,
        "Open Downloads Folder",
        true,
        None::<&str>,
    )?;
    let open_latest_download = MenuItem::with_id(
        app,
        MENU_OPEN_LATEST_DOWNLOAD,
        "Open Latest Download",
        true,
        None::<&str>,
    )?;
    let check_updates = MenuItem::with_id(
        app,
        MENU_CHECK_UPDATES,
        "Check for Updates",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "Quit", true, None::<&str>)?;

    Menu::with_items(
        app,
        &[
            &show_window,
            &new_chat,
            &reload,
            &PredefinedMenuItem::separator(app)?,
            &open_in_browser,
            &open_downloads,
            &open_latest_download,
            &PredefinedMenuItem::separator(app)?,
            autostart_item,
            &check_updates,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )
}

pub(crate) fn build_tray_state(app: &AppHandle) -> tauri::Result<TrayState> {
    let autostart_item = CheckMenuItem::with_id(
        app,
        MENU_TOGGLE_AUTOSTART,
        "Launch at Login",
        true,
        is_autostart_enabled(app),
        None::<&str>,
    )?;
    let tray_menu = build_tray_menu(app, &autostart_item)?;
    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&tray_menu)
        .tooltip(app_title(app))
        .show_menu_on_left_click(false);

    if let Some(icon) = app.default_window_icon().cloned() {
        tray = tray.icon(icon);
    }

    Ok(TrayState {
        _tray: tray.build(app)?,
        autostart_item,
    })
}

pub(crate) fn handle_tray_icon_event(app: &AppHandle, event: TrayIconEvent) {
    match event {
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        }
        | TrayIconEvent::DoubleClick {
            button: MouseButton::Left,
            ..
        } => show_main_window_from_app(app),
        _ => {}
    }
}
