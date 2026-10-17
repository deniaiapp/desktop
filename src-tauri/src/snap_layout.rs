//! Windows 11 Snap Layouts for the custom maximize button.
//!
//! Windows shows the Snap Layouts flyout only over a window that answers
//! `WM_NCHITTEST` with `HTMAXBUTTON`. The title bar's maximize button is HTML
//! inside a WebView2 child window owned by another process, so the main
//! window never sees hit tests there. Instead, a small native child window
//! sits on top of the button and claims to be the maximize button. It never
//! paints, so the HTML button stays visible, but it now owns the mouse in that
//! rectangle: it reports hover and press back to the title bar page and
//! performs the maximize or restore itself.

use std::sync::atomic::{AtomicIsize, Ordering};

use tauri::{AppHandle, Manager, Window};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetStockObject, HBRUSH, NULL_BRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TrackMouseEvent, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetClientRect, GetParent, GetWindowLongPtrW, IsZoomed,
    LoadCursorW, PostMessageW, RegisterClassExW, SetWindowLongPtrW, SetWindowPos, CREATESTRUCTW,
    GWLP_USERDATA, HTMAXBUTTON, HWND_TOP, IDC_ARROW, SC_MAXIMIZE, SC_RESTORE, SWP_HIDEWINDOW,
    SWP_NOACTIVATE, SWP_SHOWWINDOW, WINDOW_EX_STYLE, WM_CAPTURECHANGED, WM_LBUTTONUP, WM_MOUSEMOVE,
    WM_NCCREATE, WM_NCDESTROY, WM_NCHITTEST, WM_NCLBUTTONDOWN, WM_NCLBUTTONUP, WM_NCMOUSELEAVE,
    WM_NCMOUSEMOVE, WM_SYSCOMMAND, WNDCLASSEXW, WS_CHILD, WS_CLIPSIBLINGS, WS_VISIBLE,
};

use crate::titlebar::{set_maximize_button_state, CAPTION_BUTTON_WIDTH, TITLEBAR_HEIGHT};

const OVERLAY_CLASS: PCWSTR = w!("DeniSnapLayoutsOverlay");

/// The overlay's HWND, or 0 before it is created.
static OVERLAY: AtomicIsize = AtomicIsize::new(0);

/// Per-overlay state, stored in `GWLP_USERDATA`.
struct OverlayState {
    app: AppHandle,
    hover: bool,
    pressed: bool,
}

fn overlay_hwnd() -> Option<HWND> {
    match OVERLAY.load(Ordering::SeqCst) {
        0 => None,
        raw => Some(HWND(raw as *mut _)),
    }
}

/// Creates the overlay over the main window's maximize button. Must run on
/// the main thread so the overlay shares the main window's message loop.
pub(crate) fn install_overlay(window: &Window) -> Result<(), String> {
    let parent = window.hwnd().map_err(|error| error.to_string())?;
    let instance = unsafe { GetModuleHandleW(None) }.map_err(|error| error.to_string())?;

    let class = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(overlay_proc),
        hInstance: instance.into(),
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW) }.unwrap_or_default(),
        // Never paint: the HTML button underneath shows through.
        hbrBackground: HBRUSH(unsafe { GetStockObject(NULL_BRUSH) }.0),
        lpszClassName: OVERLAY_CLASS,
        ..Default::default()
    };
    // Registration fails harmlessly if the class already exists.
    unsafe { RegisterClassExW(&class) };

    let state = Box::into_raw(Box::new(OverlayState {
        app: window.app_handle().clone(),
        hover: false,
        pressed: false,
    }));
    // No extended styles: WS_EX_LAYERED or WS_EX_TRANSPARENT would make Windows
    // skip the overlay when hit testing. WS_CLIPSIBLINGS keeps the webview
    // siblings from claiming its rectangle.
    let overlay = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            OVERLAY_CLASS,
            w!(""),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
            0,
            0,
            0,
            0,
            Some(parent),
            None,
            Some(instance.into()),
            Some(state as *const _),
        )
    };
    let overlay = match overlay {
        Ok(overlay) => overlay,
        Err(error) => {
            drop(unsafe { Box::from_raw(state) });
            return Err(error.to_string());
        }
    };

    OVERLAY.store(overlay.0 as isize, Ordering::SeqCst);
    place_overlay(window);
    Ok(())
}

/// Moves the overlay over the maximize button, which is the second window
/// control from the right edge of the title bar.
pub(crate) fn place_overlay(window: &Window) {
    let Some(overlay) = overlay_hwnd() else {
        return;
    };
    let size = window.inner_size().unwrap_or_default();
    let scale = window.scale_factor().unwrap_or(1.0);
    let button = (CAPTION_BUTTON_WIDTH * scale).round() as i32;
    let height = (TITLEBAR_HEIGHT * scale).round() as i32;
    let x = size.width as i32 - 2 * button;

    // Re-raise above the webviews every time; they are siblings.
    let visibility = if x < 0 || size.height == 0 {
        SWP_HIDEWINDOW
    } else {
        SWP_SHOWWINDOW
    };
    let _ = unsafe {
        SetWindowPos(
            overlay,
            Some(HWND_TOP),
            x.max(0),
            0,
            button,
            height,
            SWP_NOACTIVATE | visibility,
        )
    };
}

unsafe fn overlay_state<'a>(hwnd: HWND) -> Option<&'a mut OverlayState> {
    let raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut OverlayState;
    raw.as_mut()
}

fn publish(state: &mut OverlayState, hover: bool, pressed: bool) {
    if state.hover != hover || state.pressed != pressed {
        state.hover = hover;
        state.pressed = pressed;
        set_maximize_button_state(&state.app, hover, pressed);
    }
}

/// Whether a client-area mouse message's position lies inside the overlay.
fn contains_client_point(hwnd: HWND, lparam: LPARAM) -> bool {
    let point = POINT {
        x: (lparam.0 & 0xFFFF) as i16 as i32,
        y: ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
    };
    let mut rect = RECT::default();
    unsafe { GetClientRect(hwnd, &mut rect) }.is_ok()
        && point.x >= rect.left
        && point.x < rect.right
        && point.y >= rect.top
        && point.y < rect.bottom
}

/// Maximizes or restores the main window the same way its caption button
/// would. Posted so the size change happens outside this window procedure.
fn toggle_parent_maximize(hwnd: HWND) {
    let Ok(parent) = (unsafe { GetParent(hwnd) }) else {
        return;
    };
    let command = if unsafe { IsZoomed(parent) }.as_bool() {
        SC_RESTORE
    } else {
        SC_MAXIMIZE
    };
    let _ = unsafe {
        PostMessageW(
            Some(parent),
            WM_SYSCOMMAND,
            WPARAM(command as usize),
            LPARAM(0),
        )
    };
}

unsafe extern "system" fn overlay_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCCREATE => {
            let create = lparam.0 as *const CREATESTRUCTW;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*create).lpCreateParams as isize);
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_NCDESTROY => {
            let raw = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *mut OverlayState;
            if !raw.is_null() {
                drop(Box::from_raw(raw));
            }
            let _ =
                OVERLAY.compare_exchange(hwnd.0 as isize, 0, Ordering::SeqCst, Ordering::SeqCst);
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        // This is what makes Windows 11 offer the Snap Layouts flyout.
        WM_NCHITTEST => LRESULT(HTMAXBUTTON as isize),
        WM_NCMOUSEMOVE => {
            if let Some(state) = overlay_state(hwnd) {
                let pressed = state.pressed;
                publish(state, true, pressed);
                // Ask for WM_NCMOUSELEAVE so the hover state clears.
                let mut track = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE | TME_NONCLIENT,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                let _ = TrackMouseEvent(&mut track);
            }
            // Default handling drives the flyout's hover timer.
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_NCMOUSELEAVE => {
            if let Some(state) = overlay_state(hwnd) {
                // While pressed the mouse is captured; leaving is tracked by
                // WM_MOUSEMOVE instead.
                if !state.pressed {
                    publish(state, false, false);
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_NCLBUTTONDOWN => {
            if let Some(state) = overlay_state(hwnd) {
                publish(state, true, true);
            }
            // Capture so the release arrives here even off the button.
            SetCapture(hwnd);
            // Skip DefWindowProc: its caption-button tracking would act on
            // this child window rather than the main window.
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            if let Some(state) = overlay_state(hwnd) {
                if state.pressed {
                    let inside = contains_client_point(hwnd, lparam);
                    publish(state, inside, true);
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONUP | WM_NCLBUTTONUP => {
            let released_inside = match overlay_state(hwnd) {
                Some(state) if state.pressed => {
                    let inside = msg == WM_NCLBUTTONUP || contains_client_point(hwnd, lparam);
                    // The button usually moves away as the window resizes;
                    // the next WM_NCMOUSEMOVE restores hover if it did not.
                    publish(state, false, false);
                    Some(inside)
                }
                _ => None,
            };
            // ReleaseCapture re-enters this procedure with WM_CAPTURECHANGED,
            // so the state borrow above has to end first.
            if let Some(inside) = released_inside {
                let _ = ReleaseCapture();
                if inside {
                    toggle_parent_maximize(hwnd);
                }
            }
            LRESULT(0)
        }
        WM_CAPTURECHANGED => {
            if let Some(state) = overlay_state(hwnd) {
                if state.pressed {
                    publish(state, false, false);
                }
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
