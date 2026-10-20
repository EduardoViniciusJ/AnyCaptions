use tauri::{LogicalPosition, LogicalSize, WebviewWindowBuilder, webview::NewWindowResponse};

#[derive(Debug, PartialEq)]
struct Geometry {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
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
        let geometry = initial_geometry(position.x, position.y, size.width, size.height);
        window.set_min_size(Some(LogicalSize::new(
            320.0_f64.min(size.width),
            160.0_f64.min(size.height),
        )))?;
        window.set_size(LogicalSize::new(geometry.width, geometry.height))?;
        window.set_position(LogicalPosition::new(geometry.x, geometry.y))?;
    }
    window.set_ignore_cursor_events(false)?;
    window.show()?;
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
}
