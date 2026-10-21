use tauri::Manager;
use tauri::{PhysicalPosition, PhysicalSize, WebviewWindowBuilder, webview::NewWindowResponse};

use crate::preferences::{PreferenceStore, SavedGeometry};

#[derive(Debug, PartialEq)]
struct Geometry {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Debug)]
struct WorkArea {
    name: Option<String>,
    scale: f64,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

fn restored_geometry<'a>(
    saved: &SavedGeometry,
    areas: &'a [WorkArea],
) -> Option<(Geometry, &'a WorkArea)> {
    let area = areas
        .iter()
        .find(|area| area.name == saved.monitor)
        .or_else(|| areas.first())?;
    let width = saved.width.max(320.0_f64.min(area.width)).min(area.width);
    let height = saved
        .height
        .max(160.0_f64.min(area.height))
        .min(area.height);
    let x = area.x + (area.width - width).max(0.0) * saved.relative_x.clamp(0.0, 1.0);
    let y = area.y + (area.height - height).max(0.0) * saved.relative_y.clamp(0.0, 1.0);
    Some((
        Geometry {
            x,
            y,
            width,
            height,
        },
        area,
    ))
}

fn initial_geometry(x: f64, y: f64, width: f64, height: f64) -> Geometry {
    let window_width = 800.0_f64.min(width);
    let window_height = 220.0_f64.min(height);
    let margin = 32.0_f64.min((height - window_height).max(0.0));
    Geometry {
        x: x + (width - window_width) / 2.0,
        y: y + height - window_height - margin,
        width: window_width,
        height: window_height,
    }
}

fn navigation_allowed(url: &tauri::Url) -> bool {
    if url.scheme() != "http" || !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    let packaged =
        url.host_str() == Some("tauri.localhost") && url.port_or_known_default() == Some(80);
    let development = cfg!(dev) && url.host_str() == Some("127.0.0.1") && url.port() == Some(1420);
    packaged || development
}

pub fn setup(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|config| config.label == "overlay")
        .ok_or_else(|| std::io::Error::other("Missing overlay configuration"))?;
    let window = WebviewWindowBuilder::from_config(app, config)?
        .on_navigation(navigation_allowed)
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .build()?;
    if let Some(monitor) = window.primary_monitor()? {
        let scale = monitor.scale_factor();
        let area = monitor.work_area();
        let position = area.position.to_logical::<f64>(scale);
        let size = area.size.to_logical::<f64>(scale);
        let initial = WorkArea {
            name: monitor.name().cloned(),
            scale,
            x: position.x,
            y: position.y,
            width: size.width,
            height: size.height,
        };
        let mut areas = vec![initial];
        for other in window.available_monitors()? {
            if other.name() == monitor.name() {
                continue;
            }
            let work = other.work_area();
            let factor = other.scale_factor();
            let pos = work.position.to_logical::<f64>(factor);
            let size = work.size.to_logical::<f64>(factor);
            areas.push(WorkArea {
                name: other.name().cloned(),
                scale: factor,
                x: pos.x,
                y: pos.y,
                width: size.width,
                height: size.height,
            });
        }
        let saved = app.state::<PreferenceStore>().snapshot()?.geometry;
        let (geometry, selected) = saved
            .as_ref()
            .and_then(|saved| restored_geometry(saved, &areas))
            .unwrap_or_else(|| {
                (
                    initial_geometry(position.x, position.y, size.width, size.height),
                    &areas[0],
                )
            });
        window.set_min_size(Some(PhysicalSize::new(
            (320.0_f64.min(selected.width) * selected.scale).round() as u32,
            (160.0_f64.min(selected.height) * selected.scale).round() as u32,
        )))?;
        window.set_size(PhysicalSize::new(
            (geometry.width * selected.scale).round() as u32,
            (geometry.height * selected.scale).round() as u32,
        ))?;
        window.set_position(PhysicalPosition::new(
            (geometry.x * selected.scale).round() as i32,
            (geometry.y * selected.scale).round() as i32,
        ))?;
    }
    window.set_ignore_cursor_events(false)?;
    window.show()?;
    Ok(())
}

pub fn capture_geometry<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
) -> tauri::Result<Option<SavedGeometry>> {
    let Some(monitor) = window.current_monitor()? else {
        return Ok(None);
    };
    let factor = monitor.scale_factor();
    let work = monitor.work_area();
    let origin = work.position.to_logical::<f64>(factor);
    let area = work.size.to_logical::<f64>(factor);
    let pos = window.outer_position()?.to_logical::<f64>(factor);
    let size = window.outer_size()?.to_logical::<f64>(factor);
    let width = size.width.min(area.width);
    let height = size.height.min(area.height);
    Ok(Some(SavedGeometry {
        monitor: monitor.name().cloned(),
        relative_x: ((pos.x - origin.x) / (area.width - width).max(1.0)).clamp(0.0, 1.0),
        relative_y: ((pos.y - origin.y) / (area.height - height).max(1.0)).clamp(0.0, 1.0),
        width,
        height,
    }))
}

pub fn ensure_visible<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) -> tauri::Result<()> {
    let pos = window.outer_position()?;
    let size = window.outer_size()?;
    let visible = window.available_monitors()?.iter().any(|monitor| {
        let work = monitor.work_area();
        let left = pos.x.max(work.position.x);
        let top = pos.y.max(work.position.y);
        let right = (pos.x + size.width as i32).min(work.position.x + work.size.width as i32);
        let bottom = (pos.y + size.height as i32).min(work.position.y + work.size.height as i32);
        right - left >= 32 && bottom - top >= 32
    });
    if !visible && let Some(monitor) = window.primary_monitor()? {
        let area = monitor.work_area();
        let scale = monitor.scale_factor();
        let pos = area.position.to_logical::<f64>(scale);
        let size = area.size.to_logical::<f64>(scale);
        let geometry = initial_geometry(pos.x, pos.y, size.width, size.height);
        window.set_min_size(Some(PhysicalSize::new(
            (320.0_f64.min(size.width) * scale).round() as u32,
            (160.0_f64.min(size.height) * scale).round() as u32,
        )))?;
        window.set_size(PhysicalSize::new(
            (geometry.width * scale).round() as u32,
            (geometry.height * scale).round() as u32,
        ))?;
        window.set_position(PhysicalPosition::new(
            (geometry.x * scale).round() as i32,
            (geometry.y * scale).round() as i32,
        ))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_accepts_only_the_app_origin_and_exact_development_server() {
        assert!(navigation_allowed(
            &"http://tauri.localhost/".parse().unwrap()
        ));
        assert_eq!(
            navigation_allowed(&"http://127.0.0.1:1420/".parse().unwrap()),
            cfg!(dev)
        );
        for url in [
            "https://example.com",
            "http://tauri.localhost.example.com",
            "http://127.0.0.1:9999",
            "file:///C:/Windows/win.ini",
            "http://user@tauri.localhost/",
        ] {
            assert!(!navigation_allowed(&url.parse().unwrap()), "{url}");
        }
    }

    #[test]
    fn centers_near_the_bottom_of_the_work_area() {
        assert_eq!(
            initial_geometry(0.0, 0.0, 1920.0, 1040.0),
            Geometry {
                x: 560.0,
                y: 788.0,
                width: 800.0,
                height: 220.0
            }
        );
    }

    #[test]
    fn handles_negative_monitor_coordinates_and_small_work_areas() {
        assert_eq!(
            initial_geometry(-1280.0, 0.0, 1280.0, 720.0),
            Geometry {
                x: -1040.0,
                y: 468.0,
                width: 800.0,
                height: 220.0
            }
        );
        assert_eq!(
            initial_geometry(0.0, 0.0, 300.0, 150.0),
            Geometry {
                x: 0.0,
                y: 0.0,
                width: 300.0,
                height: 150.0
            }
        );
    }

    #[test]
    fn dimensions_are_logical_at_high_dpi() {
        let geometry = initial_geometry(0.0, 0.0, 1920.0 / 1.5, 1080.0 / 1.5);
        assert_eq!(geometry.width, 800.0);
        assert!(geometry.y + geometry.height <= 720.0);
    }

    #[test]
    fn saved_position_falls_back_when_a_monitor_disappears() {
        let areas = [WorkArea {
            name: Some("primary".into()),
            scale: 1.0,
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        }];
        let saved = SavedGeometry {
            monitor: Some("disconnected".into()),
            relative_x: 0.5,
            relative_y: 1.0,
            width: 800.0,
            height: 220.0,
        };
        let (restored, _) = restored_geometry(&saved, &areas).unwrap();
        assert_eq!(
            restored,
            Geometry {
                x: 240.0,
                y: 500.0,
                width: 800.0,
                height: 220.0
            }
        );
    }

    #[test]
    fn saved_relative_position_scales_with_the_monitor_work_area() {
        let areas = [WorkArea {
            name: Some("primary".into()),
            scale: 1.5,
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        }];
        let saved = SavedGeometry {
            monitor: Some("primary".into()),
            relative_x: 0.5,
            relative_y: 1.0,
            width: 1000.0,
            height: 250.0,
        };
        let (restored, _) = restored_geometry(&saved, &areas).unwrap();
        assert_eq!(
            restored,
            Geometry {
                x: 140.0,
                y: 470.0,
                width: 1000.0,
                height: 250.0
            }
        );
    }
}
