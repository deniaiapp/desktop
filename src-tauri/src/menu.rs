use tauri::{
    menu::{AboutMetadataBuilder, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
    AppHandle, Manager, Wry,
};

use crate::state::DesktopState;
use crate::updater::check_for_updates;
use crate::webview::{
    adjust_zoom, hide_main_window, navigate_history, open_current_page_in_browser,
    open_downloads_folder, open_latest_download, reload_current_page, retry_current_page, set_zoom,
    show_main_window_from_app,
};

pub(crate) const MENU_SHOW_WINDOW: &str = "show-window";
pub(crate) const MENU_HIDE_TO_TRAY: &str = "hide-to-tray";
pub(crate) const MENU_OPEN_IN_BROWSER: &str = "open-in-browser";
pub(crate) const MENU_OPEN_DOWNLOADS: &str = "open-downloads";
pub(crate) const MENU_OPEN_LATEST_DOWNLOAD: &str = "open-latest-download";
pub(crate) const MENU_NAV_BACK: &str = "nav-back";
pub(crate) const MENU_NAV_FORWARD: &str = "nav-forward";
pub(crate) const MENU_RELOAD: &str = "reload";
pub(crate) const MENU_RETRY_CONNECTION: &str = "retry-connection";
pub(crate) const MENU_ZOOM_IN: &str = "zoom-in";
pub(crate) const MENU_ZOOM_OUT: &str = "zoom-out";
pub(crate) const MENU_ZOOM_RESET: &str = "zoom-reset";
pub(crate) const MENU_CHECK_UPDATES: &str = "check-updates";
pub(crate) const MENU_QUIT: &str = "quit";

pub(crate) fn quit_app(app: &AppHandle) {
    app.state::<DesktopState>().mark_quitting();
    app.exit(0);
}

pub(crate) fn handle_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        MENU_SHOW_WINDOW => show_main_window_from_app(app),
        MENU_HIDE_TO_TRAY => hide_main_window(app),
        MENU_OPEN_IN_BROWSER => open_current_page_in_browser(app),
        MENU_OPEN_DOWNLOADS => open_downloads_folder(app),
        MENU_OPEN_LATEST_DOWNLOAD => open_latest_download(app),
        MENU_NAV_BACK => navigate_history(app, "back"),
        MENU_NAV_FORWARD => navigate_history(app, "forward"),
        MENU_RELOAD => reload_current_page(app),
        MENU_RETRY_CONNECTION => retry_current_page(app),
        MENU_ZOOM_IN => adjust_zoom(app, 0.1),
        MENU_ZOOM_OUT => adjust_zoom(app, -0.1),
        MENU_ZOOM_RESET => set_zoom(app, crate::state::DEFAULT_ZOOM_FACTOR),
        MENU_CHECK_UPDATES => check_for_updates(app.clone(), true),
        MENU_QUIT => quit_app(app),
        _ => {}
    }
}

pub(crate) fn build_app_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let show_window = MenuItem::with_id(app, MENU_SHOW_WINDOW, "Show Deni AI", true, None::<&str>)?;
    let hide_to_tray =
        MenuItem::with_id(app, MENU_HIDE_TO_TRAY, "Hide to Tray", true, Some("Ctrl+W"))?;
    let open_in_browser = MenuItem::with_id(
        app,
        MENU_OPEN_IN_BROWSER,
        "Open in Browser",
        true,
        Some("Ctrl+Shift+O"),
    )?;
    let open_downloads = MenuItem::with_id(
        app,
        MENU_OPEN_DOWNLOADS,
        "Open Downloads Folder",
        true,
        Some("Ctrl+Shift+D"),
    )?;
    let open_latest_download = MenuItem::with_id(
        app,
        MENU_OPEN_LATEST_DOWNLOAD,
        "Open Latest Download",
        true,
        Some("Ctrl+Alt+O"),
    )?;
    let nav_back = MenuItem::with_id(app, MENU_NAV_BACK, "Back", true, Some("Alt+Left"))?;
    let nav_forward = MenuItem::with_id(app, MENU_NAV_FORWARD, "Forward", true, Some("Alt+Right"))?;
    let reload = MenuItem::with_id(app, MENU_RELOAD, "Reload", true, Some("Ctrl+R"))?;
    let retry_connection = MenuItem::with_id(
        app,
        MENU_RETRY_CONNECTION,
        "Retry Connection",
        true,
        Some("Ctrl+Shift+R"),
    )?;
    let zoom_in = MenuItem::with_id(app, MENU_ZOOM_IN, "Zoom In", true, Some("Ctrl+="))?;
    let zoom_out = MenuItem::with_id(app, MENU_ZOOM_OUT, "Zoom Out", true, Some("Ctrl+-"))?;
    let zoom_reset = MenuItem::with_id(app, MENU_ZOOM_RESET, "Actual Size", true, Some("Ctrl+0"))?;
    let check_updates = MenuItem::with_id(
        app,
        MENU_CHECK_UPDATES,
        "Check for Updates",
        true,
        Some("Ctrl+Shift+U"),
    )?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "Quit", true, Some("Ctrl+Q"))?;

    let about = PredefinedMenuItem::about(
        app,
        None,
        Some(
            AboutMetadataBuilder::new()
                .website(Some("https://deniai.app"))
                .website_label(Some("deniai.app"))
                .build(),
        ),
    )?;

    let file_menu = Submenu::with_items(
        app,
        "File",
        true,
        &[
            &show_window,
            &hide_to_tray,
            &PredefinedMenuItem::separator(app)?,
            &open_in_browser,
            &open_downloads,
            &open_latest_download,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    let edit_menu = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;

    let view_menu = Submenu::with_items(
        app,
        "View",
        true,
        &[
            &nav_back,
            &nav_forward,
            &PredefinedMenuItem::separator(app)?,
            &reload,
            &retry_connection,
            &PredefinedMenuItem::separator(app)?,
            &zoom_in,
            &zoom_out,
            &zoom_reset,
        ],
    )?;

    let help_menu = Submenu::with_items(
        app,
        "Help",
        true,
        &[&check_updates, &PredefinedMenuItem::separator(app)?, &about],
    )?;

    Menu::with_items(app, &[&file_menu, &edit_menu, &view_menu, &help_menu])
}
