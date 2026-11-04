//! Read-only, non-activating dictation panel, independent of the caption overlay.
use crate::dictation::runtime::DictationEventPayload;
use serde::Serialize;
use std::sync::Mutex;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};

const LABEL: &str = "dictation-indicator";
const EVENT: &str = "dictation-indicator-changed";
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    revision: u64,
    ui_language: String,
    event: DictationEventPayload,
}
#[derive(Default)]
pub struct IndicatorState(Mutex<Option<Snapshot>>);

#[derive(Clone, Copy, Debug)]
struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

fn place(anchor: Rect, screen: Rect, compact: bool) -> Rect {
    let gap = 12.0;
    let width = (if compact { 112.0_f64 } else { 640.0 }).min((screen.width - 24.0).max(1.0));
    let height = (if compact { 56.0_f64 } else { 136.0 }).min(screen.height);
    let x =
        (anchor.x + (anchor.width - width) / 2.0).clamp(screen.x, screen.x + screen.width - width);
    let above = anchor.y - gap - height;
    let below = anchor.y + anchor.height + gap;
    let y = if above >= screen.y {
        above
    } else if below + height <= screen.y + screen.height {
        below
    } else {
        screen.y + screen.height - height
    };
    Rect {
        x,
        y,
        width,
        height,
    }
}

pub fn reposition<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let Some(panel) = app.get_webview_window(LABEL) else {
        return Ok(());
    };
    let Some(overlay) = app.get_webview_window("overlay") else {
        return Ok(());
    };
    let Some(monitor) = overlay.current_monitor()?.or(overlay.primary_monitor()?) else {
        return Ok(());
    };
    let scale = monitor.scale_factor();
    let work = monitor.work_area();
    let pos = overlay.outer_position()?.to_logical::<f64>(scale);
    let size = overlay.outer_size()?.to_logical::<f64>(scale);
    let origin = work.position.to_logical::<f64>(scale);
    let extent = work.size.to_logical::<f64>(scale);
    let compact = app
        .state::<IndicatorState>()
        .0
        .lock()
        .ok()
        .and_then(|state| state.as_ref().map(|value| !value.event.show_live_text))
        .unwrap_or(false);
    let rect = place(
        Rect {
            x: pos.x,
            y: pos.y,
            width: size.width,
            height: size.height,
        },
        Rect {
            x: origin.x,
            y: origin.y,
            width: extent.width,
            height: extent.height,
        },
        compact,
    );
    panel.set_size(PhysicalSize::new(
        (rect.width * scale).round() as u32,
        (rect.height * scale).round() as u32,
    ))?;
    panel.set_position(PhysicalPosition::new(
        (rect.x * scale).round() as i32,
        (rect.y * scale).round() as i32,
    ))
}

pub fn setup(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let panel = WebviewWindowBuilder::new(
        app,
        LABEL,
        WebviewUrl::App("index.html?view=dictation".into()),
    )
    .title("AnyCaptions — Ditado")
    .inner_size(640.0, 136.0)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .resizable(false)
    .skip_taskbar(true)
    .always_on_top(true)
    .focused(false)
    .focusable(false)
    .visible(false)
    .on_navigation(crate::window::navigation_allowed)
    .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
    .build()?;
    panel.set_ignore_cursor_events(true)?;
    reposition(app.handle())?;
    if let Some(overlay) = app.get_webview_window("overlay") {
        let handle = app.handle().clone();
        overlay.on_window_event(move |event| {
            if matches!(
                event,
                tauri::WindowEvent::Moved(_)
                    | tauri::WindowEvent::Resized(_)
                    | tauri::WindowEvent::ScaleFactorChanged { .. }
            ) {
                let _ = reposition(&handle);
            }
        });
    }
    Ok(())
}

pub fn publish<R: Runtime>(app: &AppHandle<R>, event: &DictationEventPayload) {
    let Some(state) = app.try_state::<IndicatorState>() else {
        return;
    };
    let Ok(mut cache) = state.0.lock() else {
        return;
    };
    let snapshot = Snapshot {
        revision: cache.as_ref().map_or(1, |value| value.revision + 1),
        ui_language: app
            .state::<crate::ipc::Controller>()
            .snapshot()
            .map(|s| s.ui_language.tag().to_owned())
            .unwrap_or_else(|_| "pt-br".into()),
        event: event.clone(),
    };
    *cache = Some(snapshot.clone());
    drop(cache);
    // Window operations are scheduled on the UI thread, never the audio worker.
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        // A later event may have completed the recording while this job was queued.
        let latest = handle
            .state::<IndicatorState>()
            .0
            .lock()
            .ok()
            .and_then(|value| value.clone());
        let Some(latest) = latest else {
            return;
        };
        if let Some(panel) = handle.get_webview_window(LABEL) {
            if latest.event.phase == "idle" {
                let _ = panel.hide();
            } else {
                let _ = reposition(&handle);
                let _ = panel.show();
            }
            let _ = panel.emit_to(LABEL, EVENT, latest);
        }
    });
}

#[tauri::command]
pub fn get_dictation_indicator<R: Runtime>(
    window: WebviewWindow<R>,
    state: tauri::State<'_, IndicatorState>,
) -> Result<Option<Snapshot>, String> {
    if window.label() != LABEL {
        return Err("Unauthorized window".into());
    }
    state
        .0
        .lock()
        .map(|value| value.clone())
        .map_err(|_| "Indicator unavailable".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn screen() -> Rect {
        Rect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1040.0,
        }
    }
    #[test]
    fn places_above_without_covering_captions() {
        let anchor = Rect {
            x: 500.0,
            y: 800.0,
            width: 800.0,
            height: 160.0,
        };
        let panel = place(anchor, screen(), false);
        assert!(panel.y + panel.height < anchor.y);
    }
    #[test]
    fn uses_space_below_when_overlay_is_at_top() {
        let anchor = Rect {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 160.0,
        };
        let panel = place(anchor, screen(), false);
        assert!(panel.y > anchor.y + anchor.height);
    }
    #[test]
    fn stays_on_negative_coordinate_monitor_and_small_screens() {
        let screen = Rect {
            x: -400.0,
            y: -100.0,
            width: 400.0,
            height: 500.0,
        };
        let panel = place(
            Rect {
                x: -380.0,
                y: 150.0,
                width: 320.0,
                height: 160.0,
            },
            screen,
            false,
        );
        assert!(panel.x >= screen.x && panel.x + panel.width <= screen.x + screen.width);
        assert!(panel.y >= screen.y && panel.y + panel.height <= screen.y + screen.height);
        assert_eq!(place(panel, screen, true).width, 112.0);
    }
}
