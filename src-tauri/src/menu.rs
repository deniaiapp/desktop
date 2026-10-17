use serde_json::{json, Value};
use tauri::{
    menu::{IsMenuItem, Menu, MenuEvent, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu},
    AppHandle, Manager, Wry,
};

use crate::about::open_about_window;
use crate::autostart::toggle_autostart;
use crate::edit::{run_edit_command, EditCommand};
use crate::state::DesktopState;
use crate::tray::sync_autostart_item;
use crate::updater::check_for_updates;
use crate::webview::{
    adjust_zoom, hide_main_window, navigate_history, open_current_page_in_browser,
    open_downloads_folder, open_latest_download, open_new_chat, reload_current_page,
    retry_current_page, set_zoom, show_main_window_from_app,
};

pub(crate) const MENU_SHOW_WINDOW: &str = "show-window";
pub(crate) const MENU_NEW_CHAT: &str = "new-chat";
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
pub(crate) const MENU_TOGGLE_AUTOSTART: &str = "toggle-autostart";
pub(crate) const MENU_ABOUT: &str = "about";
pub(crate) const MENU_QUIT: &str = "quit";

/// One entry in an app menu section, shared by the native menu and the
/// custom title bar menu.
pub(crate) enum MenuEntry {
    Item {
        id: &'static str,
        label: String,
        accelerator: Option<&'static str>,
    },
    Edit(EditCommand),
    Separator,
}

/// A top-level app menu such as File or View.
pub(crate) struct MenuSection {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) entries: Vec<MenuEntry>,
}

pub(crate) fn quit_app(app: &AppHandle) {
    app.state::<DesktopState>().mark_quitting();
    app.exit(0);
}

fn item(
    id: &'static str,
    label: impl Into<String>,
    accelerator: Option<&'static str>,
) -> MenuEntry {
    MenuEntry::Item {
        id,
        label: label.into(),
        accelerator,
    }
}

pub(crate) fn app_menu_sections(app: &AppHandle) -> Vec<MenuSection> {
    use MenuEntry::{Edit, Separator};

    vec![
        MenuSection {
            id: "file",
            label: "File",
            entries: vec![
                item(MENU_NEW_CHAT, "New Chat", Some("Ctrl+N")),
                Separator,
                item(MENU_SHOW_WINDOW, "Show Deni AI", None),
                item(MENU_HIDE_TO_TRAY, "Hide to Tray", Some("Ctrl+W")),
                Separator,
                item(
                    MENU_OPEN_IN_BROWSER,
                    "Open in Browser",
                    Some("Ctrl+Shift+O"),
                ),
                item(
                    MENU_OPEN_DOWNLOADS,
                    "Open Downloads Folder",
                    Some("Ctrl+Shift+D"),
                ),
                item(
                    MENU_OPEN_LATEST_DOWNLOAD,
                    "Open Latest Download",
                    Some("Ctrl+Alt+O"),
                ),
                Separator,
                item(MENU_QUIT, "Quit", Some("Ctrl+Q")),
            ],
        },
        MenuSection {
            id: "edit",
            label: "Edit",
            entries: vec![
                Edit(EditCommand::Undo),
                Edit(EditCommand::Redo),
                Separator,
                Edit(EditCommand::Cut),
                Edit(EditCommand::Copy),
                Edit(EditCommand::Paste),
                Separator,
                Edit(EditCommand::SelectAll),
            ],
        },
        MenuSection {
            id: "view",
            label: "View",
            entries: vec![
                item(MENU_NAV_BACK, "Back", Some("Alt+Left")),
                item(MENU_NAV_FORWARD, "Forward", Some("Alt+Right")),
                Separator,
                item(MENU_RELOAD, "Reload", Some("Ctrl+R")),
                item(
                    MENU_RETRY_CONNECTION,
                    "Retry Connection",
                    Some("Ctrl+Shift+R"),
                ),
                Separator,
                item(MENU_ZOOM_IN, "Zoom In", Some("Ctrl+=")),
                item(MENU_ZOOM_OUT, "Zoom Out", Some("Ctrl+-")),
                item(MENU_ZOOM_RESET, "Actual Size", Some("Ctrl+0")),
            ],
        },
        MenuSection {
            id: "help",
            label: "Help",
            entries: vec![
                item(
                    MENU_CHECK_UPDATES,
                    "Check for Updates",
                    Some("Ctrl+Shift+U"),
                ),
                Separator,
                item(
                    MENU_ABOUT,
                    format!("About {}", app.package_info().name),
                    None,
                ),
            ],
        },
    ]
}

fn menu_entry_json(entry: &MenuEntry) -> Value {
    match entry {
        MenuEntry::Item {
            id,
            label,
            accelerator,
        } => json!({ "type": "item", "id": id, "label": label, "accelerator": accelerator }),
        MenuEntry::Edit(command) => json!({
            "type": "item",
            "id": command.id(),
            "label": command.label(),
            "accelerator": command.accelerator(),
        }),
        MenuEntry::Separator => json!({ "type": "separator" }),
    }
}

/// The app menu as JSON, for the custom title bar menu.
pub(crate) fn app_menu_json(app: &AppHandle) -> Value {
    app_menu_sections(app)
        .iter()
        .map(|section| {
            let entries = section
                .entries
                .iter()
                .map(menu_entry_json)
                .collect::<Vec<_>>();
            json!({ "id": section.id, "label": section.label, "entries": entries })
        })
        .collect()
}

/// Runs the action behind a menu item id, coming from either the native menu
/// or the custom title bar menu.
pub(crate) fn run_menu_command(app: &AppHandle, id: &str) {
    if let Some(command) = EditCommand::from_id(id) {
        run_edit_command(app, command);
        return;
    }

    match id {
        MENU_SHOW_WINDOW => show_main_window_from_app(app),
        MENU_NEW_CHAT => open_new_chat(app),
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
        MENU_TOGGLE_AUTOSTART => sync_autostart_item(app, toggle_autostart(app)),
        MENU_ABOUT => open_about_window(app),
        MENU_QUIT => quit_app(app),
        _ => {}
    }
}

pub(crate) fn handle_menu_event(app: &AppHandle, event: MenuEvent) {
    run_menu_command(app, event.id().as_ref());
}

fn native_edit_item(
    app: &AppHandle,
    command: EditCommand,
) -> tauri::Result<PredefinedMenuItem<Wry>> {
    match command {
        EditCommand::Undo => PredefinedMenuItem::undo(app, None),
        EditCommand::Redo => PredefinedMenuItem::redo(app, None),
        EditCommand::Cut => PredefinedMenuItem::cut(app, None),
        EditCommand::Copy => PredefinedMenuItem::copy(app, None),
        EditCommand::Paste => PredefinedMenuItem::paste(app, None),
        EditCommand::SelectAll => PredefinedMenuItem::select_all(app, None),
    }
}

fn native_menu_item(app: &AppHandle, entry: &MenuEntry) -> tauri::Result<MenuItemKind<Wry>> {
    Ok(match entry {
        MenuEntry::Item {
            id,
            label,
            accelerator,
        } => MenuItem::with_id(app, *id, label, true, *accelerator)?.kind(),
        MenuEntry::Edit(command) => native_edit_item(app, *command)?.kind(),
        MenuEntry::Separator => PredefinedMenuItem::separator(app)?.kind(),
    })
}

fn native_submenu(app: &AppHandle, section: &MenuSection) -> tauri::Result<Submenu<Wry>> {
    let items = section
        .entries
        .iter()
        .map(|entry| native_menu_item(app, entry))
        .collect::<tauri::Result<Vec<_>>>()?;
    let items = items
        .iter()
        .map(|item| item as &dyn IsMenuItem<Wry>)
        .collect::<Vec<_>>();
    Submenu::with_items(app, section.label, true, &items)
}

/// Builds the native app menu. On macOS it is the regular menu bar. On
/// Windows and Linux it stays hidden behind the custom title bar and only
/// provides the keyboard accelerators.
pub(crate) fn build_app_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let submenus = app_menu_sections(app)
        .iter()
        .map(|section| native_submenu(app, section))
        .collect::<tauri::Result<Vec<_>>>()?;
    let submenus = submenus
        .iter()
        .map(|submenu| submenu as &dyn IsMenuItem<Wry>)
        .collect::<Vec<_>>();
    Menu::with_items(app, &submenus)
}
