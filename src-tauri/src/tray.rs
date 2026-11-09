//! System tray icon and its menu (recent links, settings, quit).

use crate::{AppState, app_windows, open_in_picker};
use tauri::{
    AppHandle, Manager,
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

const TRAY_ID: &str = "main";
const RECENTS_IN_MENU: usize = 10;
const MAX_LABEL_CHARS: usize = 56;

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(tauri::include_image!("icons/tray.png"))
        .tooltip("PickMyBrowser")
        .menu(&build_menu(app)?)
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .on_menu_event(|app, event| on_menu_item(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                app_windows::show_settings(tray.app_handle());
            }
        })
        .build(app)?;
    tray.set_visible(show_tray_setting(app))?;
    Ok(())
}

/// Rebuilds the menu so it reflects the current recents list.
pub fn refresh(app: &AppHandle) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id(TRAY_ID), build_menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
}

pub fn set_visible(app: &AppHandle, visible: bool) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_visible(visible);
    }
}

fn show_tray_setting(app: &AppHandle) -> bool {
    app.state::<AppState>().cfg.lock().unwrap().settings.show_tray
}

fn recent_url(app: &AppHandle, index: usize) -> Option<String> {
    let state = app.state::<AppState>();
    let cfg = state.cfg.lock().unwrap();
    cfg.recents.get(index).map(|r| r.url.clone())
}

fn on_menu_item(app: &AppHandle, id: &str) {
    let recent_index = match id {
        "quit" => return app.exit(0),
        "settings" => return app_windows::show_settings(app),
        "update" => {
            app_windows::show_settings(app);
            let _ = tauri::Emitter::emit(app, "settings:tab", "general");
            return;
        }
        "reopen" => Some(0),
        _ => id.strip_prefix("recent:").and_then(|i| i.parse().ok()),
    };
    if let Some(url) = recent_index.and_then(|i| recent_url(app, i)) {
        open_in_picker(app, &url);
    }
}

fn menu_label(url: &str) -> String {
    let label = url.trim_start_matches("https://").trim_start_matches("http://");
    if label.chars().count() > MAX_LABEL_CHARS {
        let truncated: String = label.chars().take(MAX_LABEL_CHARS - 1).collect();
        truncated + "…"
    } else {
        label.to_string()
    }
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let recents = app.state::<AppState>().cfg.lock().unwrap().recents.clone();
    let has_recents = !recents.is_empty();

    let recents_menu = Submenu::with_id(app, "recents", "Recent links", has_recents)?;
    for (i, recent) in recents.iter().take(RECENTS_IN_MENU).enumerate() {
        let item = MenuItem::with_id(app, format!("recent:{i}"), menu_label(&recent.url), true, None::<&str>)?;
        recents_menu.append(&item)?;
    }

    let reopen = MenuItem::with_id(app, "reopen", "Reopen last link…", has_recents, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit PickMyBrowser", true, None::<&str>)?;
    let separator_1 = PredefinedMenuItem::separator(app)?;
    let separator_2 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&reopen, &recents_menu, &separator_1, &settings, &separator_2, &quit])?;
    // A found update is offered here too; it opens Settings, which asks before installing.
    if let Some(update) = crate::updates::available(app) {
        let label = format!("Update to v{} available…", update.version);
        menu.insert(&MenuItem::with_id(app, "update", label, true, None::<&str>)?, 0)?;
        menu.insert(&PredefinedMenuItem::separator(app)?, 1)?;
    }
    Ok(menu)
}
