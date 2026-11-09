//! The two webview windows: the frameless link picker and the settings window.
//! Both are created lazily and only ever hidden, never closed, so reopening is instant.

use crate::config;
use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};

pub const PICKER: &str = "picker";
const SETTINGS: &str = "settings";

/// Returns the picker window, creating it (hidden) on first use.
pub fn picker(app: &AppHandle) -> WebviewWindow {
    if let Some(window) = app.get_webview_window(PICKER) {
        return window;
    }
    let window = WebviewWindowBuilder::new(app, PICKER, WebviewUrl::App("index.html".into()))
        .title("PickMyBrowser")
        .inner_size(460.0, 320.0)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .focused(true)
        .build()
        .expect("picker window");

    let handle = window.clone();
    window.on_window_event(move |event| match event {
        // Under WebDriver the window loses focus constantly, so it must stay up in E2E.
        WindowEvent::Focused(false) if !config::is_e2e() => {
            let _ = handle.hide();
        }
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            let _ = handle.hide();
        }
        _ => {}
    });
    window
}

pub fn hide_picker(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(PICKER) {
        let _ = window.hide();
    }
}

/// Sizes the picker to the measured content and places it near the cursor (or centred)
/// on the monitor under the cursor, kept inside that monitor's work area.
pub fn present_picker(app: &AppHandle, width: f64, height: f64, centered: bool) -> Result<(), String> {
    let window = picker(app);
    let cursor = app.cursor_position().map_err(|e| e.to_string())?;
    let monitor = app
        .monitor_from_point(cursor.x, cursor.y)
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten());
    let scale = monitor.as_ref().map(|m| m.scale_factor()).unwrap_or(1.0);
    let size = PhysicalSize::new((width * scale).round(), (height * scale).round());
    let _ = window.set_size(size);

    if let Some(monitor) = monitor {
        let area = monitor.work_area();
        let left = area.position.x as f64;
        let top = area.position.y as f64;
        let area_width = area.size.width as f64;
        let area_height = area.size.height as f64;

        let (x, y) = if centered {
            // A third of the way down reads as "centred" better than true centre.
            (left + (area_width - size.width) / 2.0, top + (area_height - size.height) / 3.0)
        } else {
            (cursor.x - 24.0 * scale, cursor.y - 20.0 * scale)
        };

        let margin = 8.0 * scale;
        let max_x = (left + area_width - size.width - margin).max(left + margin);
        let max_y = (top + area_height - size.height - margin).max(top + margin);
        let x = x.clamp(left + margin, max_x);
        let y = y.clamp(top + margin, max_y);
        let _ = window.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
    }

    window.show().map_err(|e| e.to_string())?;
    let _ = window.set_focus();
    Ok(())
}

pub fn show_settings(app: &AppHandle) {
    let window = match app.get_webview_window(SETTINGS) {
        Some(window) => window,
        None => create_settings(app),
    };
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}

fn create_settings(app: &AppHandle) -> WebviewWindow {
    let window = WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::App("index.html".into()))
        .title("PickMyBrowser")
        .inner_size(760.0, 540.0)
        .min_inner_size(640.0, 460.0)
        .center()
        .visible(false)
        .build()
        .expect("settings window");

    let handle = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = handle.hide();
        }
    });
    window
}
