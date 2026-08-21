//! System tray setup and managed tray state.

use std::{
    error::Error,
    io,
    sync::{
        atomic::{AtomicBool, Ordering},
        RwLock,
    },
};

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

/// Shared state for dynamically updating the system tray during scans.
pub struct TrayState {
    pub summary_item: MenuItem<tauri::Wry>,
    pub scan_item: MenuItem<tauri::Wry>,
    pub tray_icon: tauri::tray::TrayIcon<tauri::Wry>,
    pub is_scanning: AtomicBool,
    // Favor concurrent tray-summary reads over infrequent writes.
    pub summary_tooltip: RwLock<String>,
}

pub fn setup(app: &mut tauri::App<tauri::Wry>) -> Result<(), Box<dyn Error>> {
    let summary_item = MenuItem::with_id(app, "summary", "All caught up", false, None::<&str>)?;
    let overview_item =
        MenuItem::with_id(app, "open-overview", "Open Overview", true, None::<&str>)?;
    let open_item = MenuItem::with_id(app, "open", "Open SiteCMD", true, None::<&str>)?;
    let scan_item = MenuItem::with_id(app, "scan-now", "Scan Now", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &summary_item,
            &overview_item,
            &open_item,
            &scan_item,
            &quit_item,
        ],
    )?;

    let tray_icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| io::Error::other("Missing default window icon for tray"))?;

    let tray = TrayIconBuilder::new()
        .icon(tray_icon)
        .tooltip("SiteCMD - All caught up")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open-overview" => {
                let _ = app.emit("tray-open-overview", ());
                focus_main_window(app, true);
            }
            "open" => focus_main_window(app, true),
            "scan-now" => handle_scan_now(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                focus_main_window(tray.app_handle(), true);
            }
        })
        .build(app)?;

    app.manage(TrayState {
        summary_item,
        scan_item,
        tray_icon: tray,
        is_scanning: AtomicBool::new(false),
        summary_tooltip: RwLock::new("SiteCMD - All caught up".to_string()),
    });

    Ok(())
}

fn handle_scan_now(app: &tauri::AppHandle) {
    if let Some(tray_state) = app.try_state::<TrayState>() {
        if tray_state.is_scanning.load(Ordering::Relaxed) {
            let _ = app.emit("tray-show-scan", ());
            focus_main_window(app, false);
            return;
        }
    }

    let _ = app.emit("tray-scan-now", ());
    focus_main_window(app, false);
}

fn focus_main_window(app: &tauri::AppHandle, unminimize: bool) {
    if let Some(window) = app.get_webview_window("main") {
        if unminimize {
            let _ = window.unminimize();
        }
        let _ = window.show();
        let _ = window.set_focus();
    }
}
