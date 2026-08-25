mod menu;
mod state;
mod tray;
mod updater;
mod url_guard;
mod webview;

use std::sync::{atomic::AtomicBool, Arc};

use tauri::{
    webview::{PageLoadEvent, Url, WebviewWindowBuilder},
    AppHandle, Manager, WindowEvent,
};

use state::{loading_title, DesktopState};
use url_guard::{is_allowed_url, APP_START_URL};
use webview::{
    handle_download_event, handle_navigation_event, handle_new_window_event,
    handle_page_load_finished, handle_page_load_started, maybe_notify_tray_behavior,
    navigate_main_window, show_main_window, show_main_window_from_app, INITIAL_SHOW_DELAY,
    MAIN_WINDOW_LABEL,
};

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let window_shown = Arc::new(AtomicBool::new(false));

    tauri::Builder::default()
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::SIZE
                        | tauri_plugin_window_state::StateFlags::POSITION
                        | tauri_plugin_window_state::StateFlags::MAXIMIZED,
                )
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            handle_second_instance(app, args);
        }))
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
            app.manage(tray::build_tray_state(&app_handle)?);

            let navigation_handle = app_handle.clone();
            let new_window_handle = app_handle.clone();
            let download_handle = app_handle.clone();
            let page_load_visible = Arc::clone(&window_shown);
            let fallback_handle = app_handle.clone();
            let fallback_visible = Arc::clone(&window_shown);
            let close_handle = app_handle.clone();

            let main_window = WebviewWindowBuilder::from_config(app.handle(), &main_window_config)?
                .on_navigation(move |url| handle_navigation_event(&navigation_handle, url))
                .on_new_window(move |url, _features| {
                    handle_new_window_event(&new_window_handle, url)
                })
                .on_download(move |_webview, event| handle_download_event(&download_handle, event))
                .on_page_load(move |window, payload| match payload.event() {
                    PageLoadEvent::Started => {
                        handle_page_load_started(&window, payload.url().to_string())
                    }
                    PageLoadEvent::Finished => handle_page_load_finished(
                        &window,
                        payload.url().to_string(),
                        &page_load_visible,
                    ),
                })
                .build()?;

            let _ = main_window.set_title(&loading_title(&app_handle));
            let close_window = main_window.clone();
            main_window.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    if !close_handle.state::<DesktopState>().is_quitting() {
                        api.prevent_close();
                        let _ = close_window.hide();
                        maybe_notify_tray_behavior(&close_handle);
                    }
                }
            });

            std::thread::spawn(move || {
                std::thread::sleep(INITIAL_SHOW_DELAY);

                if !fallback_visible.swap(true, std::sync::atomic::Ordering::SeqCst) {
                    if let Some(window) = fallback_handle.get_webview_window(MAIN_WINDOW_LABEL) {
                        let _ = window.set_title(&loading_title(&fallback_handle));
                        show_main_window(&window);
                    }
                }
            });

            updater::check_for_updates(app_handle.clone(), false);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
