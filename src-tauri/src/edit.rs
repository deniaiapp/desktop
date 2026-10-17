use tauri::AppHandle;

use crate::webview::focus_content;

/// Editing commands offered by the Edit menu.
///
/// The native menu uses the platform's predefined items for these. The custom
/// title bar menu cannot reach those, so it replays the matching shortcut into
/// the chat webview instead, which is also how the native items work on
/// Windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditCommand {
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
}

impl EditCommand {
    pub(crate) const ALL: [EditCommand; 6] = [
        EditCommand::Undo,
        EditCommand::Redo,
        EditCommand::Cut,
        EditCommand::Copy,
        EditCommand::Paste,
        EditCommand::SelectAll,
    ];

    pub(crate) fn id(self) -> &'static str {
        match self {
            EditCommand::Undo => "edit-undo",
            EditCommand::Redo => "edit-redo",
            EditCommand::Cut => "edit-cut",
            EditCommand::Copy => "edit-copy",
            EditCommand::Paste => "edit-paste",
            EditCommand::SelectAll => "edit-select-all",
        }
    }

    pub(crate) fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|command| command.id() == id)
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            EditCommand::Undo => "Undo",
            EditCommand::Redo => "Redo",
            EditCommand::Cut => "Cut",
            EditCommand::Copy => "Copy",
            EditCommand::Paste => "Paste",
            EditCommand::SelectAll => "Select All",
        }
    }

    pub(crate) fn accelerator(self) -> &'static str {
        match self {
            EditCommand::Undo => "Ctrl+Z",
            EditCommand::Redo => "Ctrl+Y",
            EditCommand::Cut => "Ctrl+X",
            EditCommand::Copy => "Ctrl+C",
            EditCommand::Paste => "Ctrl+V",
            EditCommand::SelectAll => "Ctrl+A",
        }
    }

    /// The letter pressed together with Ctrl for this command.
    #[cfg_attr(not(windows), allow(dead_code))]
    fn key(self) -> u8 {
        match self {
            EditCommand::Undo => b'Z',
            EditCommand::Redo => b'Y',
            EditCommand::Cut => b'X',
            EditCommand::Copy => b'C',
            EditCommand::Paste => b'V',
            EditCommand::SelectAll => b'A',
        }
    }

    #[cfg_attr(windows, allow(dead_code))]
    fn exec_command_name(self) -> &'static str {
        match self {
            EditCommand::Undo => "undo",
            EditCommand::Redo => "redo",
            EditCommand::Cut => "cut",
            EditCommand::Copy => "copy",
            EditCommand::Paste => "paste",
            EditCommand::SelectAll => "selectAll",
        }
    }
}

/// Gives keyboard focus back to the chat page, then runs `command` there.
pub(crate) fn run_edit_command(app: &AppHandle, command: EditCommand) {
    focus_content(app);
    send_edit_command(app, command);
}

#[cfg(windows)]
fn send_edit_command(app: &AppHandle, command: EditCommand) {
    use windows::Win32::UI::{
        Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
            KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL,
        },
        WindowsAndMessaging::GetForegroundWindow,
    };

    let Some(Ok(hwnd)) = crate::webview::main_window(app).map(|window| window.hwnd()) else {
        return;
    };
    // Raw window handles are not `Send`; compare them as integers.
    let main_hwnd = hwnd.0 as isize;

    fn key_input(key: VIRTUAL_KEY, up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: key,
                    wScan: 0,
                    dwFlags: if up {
                        KEYEVENTF_KEYUP
                    } else {
                        KEYBD_EVENT_FLAGS(0)
                    },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    // Focus moves to the chat webview asynchronously; give it a moment so the
    // shortcut lands there rather than in the menu popup that just closed.
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(60));
        // The keystrokes go to whatever has focus. Never type into another app.
        if unsafe { GetForegroundWindow() }.0 as isize != main_hwnd {
            eprintln!("skipped {:?}: Deni AI is not in the foreground", command);
            return;
        }
        let key = VIRTUAL_KEY(u16::from(command.key()));
        let inputs = [
            key_input(VK_CONTROL, false),
            key_input(key, false),
            key_input(key, true),
            key_input(VK_CONTROL, true),
        ];
        let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
        if sent as usize != inputs.len() {
            eprintln!("failed to send {:?} shortcut", command);
        }
    });
}

#[cfg(not(windows))]
fn send_edit_command(app: &AppHandle, command: EditCommand) {
    let Some(webview) = crate::webview::content_webview(app) else {
        return;
    };
    let script = format!("document.execCommand({:?});", command.exec_command_name());
    if let Err(error) = webview.eval(script) {
        eprintln!("failed to run {:?}: {}", command, error);
    }
}
