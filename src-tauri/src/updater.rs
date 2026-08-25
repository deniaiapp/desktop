use tauri::AppHandle;
use tauri_plugin_updater::{Error as UpdaterError, UpdaterExt};

use crate::state::app_title;
use crate::webview::notify;

pub(crate) fn check_for_updates(app: AppHandle, interactive: bool) {
    tauri::async_runtime::spawn(async move {
        let updater = match app.updater() {
            Ok(updater) => updater,
            Err(error) => {
                if interactive {
                    notify(
                        &app,
                        app_title(&app),
                        format!("Couldn't start the updater: {error}"),
                    );
                } else {
                    eprintln!("failed to initialize updater: {}", error);
                }
                return;
            }
        };

        match updater.check().await {
      Ok(Some(update)) => {
        let version = update.version.clone();

        if interactive {
          notify(
            &app,
            app_title(&app),
            format!("Downloading Deni AI {version}. The installer will run when the download finishes."),
          );

          match update.download_and_install(|_, _| {}, || {}).await {
            Ok(()) => notify(
              &app,
              app_title(&app),
              format!("Deni AI {version} is ready to install."),
            ),
            Err(error) => notify(
              &app,
              app_title(&app),
              format!("The update download failed: {error}"),
            ),
          }
        } else {
          notify(
            &app,
            app_title(&app),
            format!("Deni AI {version} is available. Use Help > Check for Updates to install it."),
          );
        }
      }
      Ok(None) if interactive => {
        notify(&app, app_title(&app), "You're already on the latest desktop build.")
      }
      Ok(None) => {}
      Err(UpdaterError::EmptyEndpoints) if interactive => notify(
        &app,
        app_title(&app),
        "Updater support is wired in, but this build does not have a release feed configured yet.",
      ),
      Err(UpdaterError::EmptyEndpoints) => {}
      Err(error) if interactive => notify(
        &app,
        app_title(&app),
        format!("Couldn't check for updates: {error}"),
      ),
      Err(error) => eprintln!("background update check failed: {}", error),
    }
    });
}
