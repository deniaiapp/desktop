mod about;
mod autostart;
mod edit;
mod menu;
mod menu_popup;
mod shortcut;
#[cfg(windows)]
mod snap_layout;
mod state;
mod titlebar;
mod tray;
mod updater;
mod url_guard;
mod webview;

use std::sync::{atomic::AtomicBool, Arc};

use tauri::{
    utils::config::WindowConfig,
    webview::{PageLoadEvent, Url, WebviewBuilder},
    window::{Color, WindowBuilder},
    AppHandle, Manager, WebviewUrl, Window, WindowEvent,
};

use menu_popup::{close_menu, MenuPopupState, MENU_POPUP_LABEL};
use state::{loading_title, DesktopState};
use titlebar::{
    add_titlebar, content_bounds, layout_main_window, set_page_title, sync_titlebar, TitlebarState,
};
use url_guard::{default_app_url, is_allowed_url, APP_START_URL};
use webview::{
    content_webview, handle_download_event, handle_navigation_event, handle_new_window_event,
    handle_page_load_finished, handle_page_load_started, main_window, maybe_notify_tray_behavior,
    navigate_main_window, show_main_window, show_main_window_from_app, CONTENT_WEBVIEW_LABEL,
    INITIAL_SHOW_DELAY, MAIN_WINDOW_LABEL,
};

/// Matches the chat site's dark background so nothing flashes while it loads.
const BACKGROUND: Color = Color(10, 10, 10, 255);

fn handle_second_instance(app: &AppHandle, args: Vec<String>) {
    for arg in args {
        if let Ok(url) = arg.parse::<Url>() {
            if is_allowed_url(&url) {
                app.state::<DesktopState>().set_current_url(url.to_string());
                navigate_main_window(app, url);
                break;
            }
        }
    }

    show_main_window_from_app(app);
}

/// Builds the main window frame. Windows and Linux draw their own title bar;
/// macOS keeps its traffic lights on top of the custom one.
fn build_main_window(app: &AppHandle, config: &WindowConfig) -> tauri::Result<Window> {
    let builder = WindowBuilder::from_config(app, config)?.background_color(BACKGROUND);

    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);
    #[cfg(not(target_os = "macos"))]
    let builder = builder.decorations(false);

    builder.build()
}

fn handle_main_window_event(app: &AppHandle, window: &Window, event: &WindowEvent) {
    match event {
        WindowEvent::CloseRequested { api, .. } => {
            if !app.state::<DesktopState>().is_quitting() {
                api.prevent_close();
                close_menu(app, false);
                let _ = window.hide();
                maybe_notify_tray_behavior(app);
            }
        }
        WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
            close_menu(app, false);
            layout_main_window(window);
            sync_titlebar(app);
        }
        WindowEvent::Moved(_) => close_menu(app, false),
        WindowEvent::Focused(focused) => {
            if *focused {
                menu_popup::handle_main_window_focused(app);
            }
            sync_titlebar(app);
        }
        _ => {}
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // A login-item launch starts in the tray, so treat the first show as already done.
    let window_shown = Arc::new(AtomicBool::new(autostart::launched_by_autostart()));

    tauri::Builder::default()
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::SIZE
                        | tauri_plugin_window_state::StateFlags::POSITION
                        | tauri_plugin_window_state::StateFlags::MAXIMIZED,
                )
                .with_denylist(&[about::ABOUT_WINDOW_LABEL, MENU_POPUP_LABEL])
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(autostart::build_plugin())
        .plugin(shortcut::build_plugin())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            handle_second_instance(app, args);
        }))
        .register_uri_scheme_protocol(about::ABOUT_SCHEME, about::handle_about_protocol)
        .register_uri_scheme_protocol(titlebar::CHROME_SCHEME, titlebar::handle_chrome_protocol)
        .invoke_handler(tauri::generate_handler![
            titlebar::titlebar_ready,
            titlebar::titlebar_window,
            titlebar::titlebar_command,
            menu_popup::menu_open,
            menu_popup::menu_present,
            menu_popup::menu_step,
            menu_popup::menu_dismiss,
            menu_popup::menu_invoke,
        ])
        .menu(menu::build_app_menu)
        .on_menu_event(menu::handle_menu_event)
        .on_tray_icon_event(tray::handle_tray_icon_event)
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let main_window_config = app
                .config()
                .app
                .windows
                .iter()
                .find(|window| window.label == MAIN_WINDOW_LABEL)
                .cloned()
                .expect("main window config must exist");

            app.manage(DesktopState::new(
                main_window_config.title.clone(),
                APP_START_URL,
            ));
            app.manage(TitlebarState::new());
            app.manage(MenuPopupState::default());
            app.manage(tray::build_tray_state(&app_handle)?);

            let navigation_handle = app_handle.clone();
            let new_window_handle = app_handle.clone();
            let download_handle = app_handle.clone();
            let page_load_visible = Arc::clone(&window_shown);
            let fallback_handle = app_handle.clone();
            let fallback_visible = Arc::clone(&window_shown);
            let event_handle = app_handle.clone();

            let frame = build_main_window(&app_handle, &main_window_config)?;
            let content = WebviewBuilder::new(
                CONTENT_WEBVIEW_LABEL,
                WebviewUrl::External(default_app_url()),
            )
            .background_color(BACKGROUND)
            .on_navigation(move |url| handle_navigation_event(&navigation_handle, url))
            .on_new_window(move |url, _features| handle_new_window_event(&new_window_handle, url))
            .on_download(move |_webview, event| handle_download_event(&download_handle, event))
            .on_document_title_changed(|webview, title| set_page_title(webview.app_handle(), title))
            .on_page_load(move |webview, payload| match payload.event() {
                PageLoadEvent::Started => {
                    handle_page_load_started(&webview, payload.url().to_string())
                }
                PageLoadEvent::Finished => handle_page_load_finished(
                    &webview,
                    payload.url().to_string(),
                    &page_load_visible,
                ),
            });
            let bounds = content_bounds(&frame);
            frame.add_child(content, bounds.position, bounds.size)?;
            add_titlebar(&frame)?;

            #[cfg(not(target_os = "macos"))]
            {
                // The custom title bar draws the menus; the hidden native menu
                // still provides the keyboard accelerators.
                let _ = frame.hide_menu();
                // Child webviews cover the whole client area. Re-applying
                // `resizable` puts the invisible resize borders on top of them.
                let _ = frame.set_resizable(true);
                menu_popup::create_menu_popup(&app_handle, &frame)?;
            }
            #[cfg(windows)]
            if let Err(error) = snap_layout::install_overlay(&frame) {
                eprintln!("failed to install the Snap Layouts overlay: {}", error);
            }

            let _ = frame.set_title(&loading_title(&app_handle));
            let event_window = frame.clone();
            frame.on_window_event(move |event| {
                handle_main_window_event(&event_handle, &event_window, event)
            });

            std::thread::spawn(move || {
                std::thread::sleep(INITIAL_SHOW_DELAY);

                if let Some(window) = main_window(&fallback_handle) {
                    if !fallback_visible.swap(true, std::sync::atomic::Ordering::SeqCst) {
                        let _ = window.set_title(&loading_title(&fallback_handle));
                        show_main_window(&window);
                    }
                }
                if let Some(content) = content_webview(&fallback_handle) {
                    if !content.url().is_ok_and(|url| is_allowed_url(&url)) {
                        navigate_main_window(&fallback_handle, default_app_url());
                    }
                }
            });

            shortcut::register_global_shortcuts(&app_handle);
            updater::check_for_updates(app_handle.clone(), false);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
