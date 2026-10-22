use serde_json::{Value, json};
use tauri::{
    WebviewUrl, WebviewWindowBuilder,
    ipc::{CallbackFn, InvokeBody},
    test::{MockRuntime, get_ipc_response, mock_builder},
    webview::InvokeRequest,
};

fn invoke(
    window: &tauri::WebviewWindow<MockRuntime>,
    command: &str,
    body: Value,
    url: &str,
) -> Result<Value, Value> {
    get_ipc_response(
        window,
        InvokeRequest {
            cmd: command.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: url.parse().unwrap(),
            body: InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.to_string(),
        },
    )
    .map(|response| response.deserialize().unwrap())
}

fn app() -> tauri::App<MockRuntime> {
    anycaptions_lib::builder(mock_builder())
        .build(tauri::generate_context!())
        .unwrap()
}

#[test]
fn local_overlay_can_query_and_change_mode_but_cannot_start_without_resources() {
    let app = app();
    let window = WebviewWindowBuilder::new(&app, "overlay", WebviewUrl::default())
        .build()
        .unwrap();
    let url = "http://tauri.localhost";
    let state = invoke(&window, "get_state", json!({}), url).unwrap();
    assert_eq!(state["capture"], "stopped");
    assert_eq!(state["windowMode"], "edit");
    let changed = invoke(
        &window,
        "set_caption_mode",
        json!({"request":{"mode":"en-pt"}}),
        url,
    )
    .unwrap();
    assert_eq!(changed["mode"], "en-pt");
    assert_eq!(changed["sessionId"], 1);
    assert!(invoke(&window, "start_capture", json!({}), url).is_err());
    assert!(
        invoke(
            &window,
            "set_appearance",
            json!({"request":{"fontSize":100,"color":"#FFFFFF","partialColor":"#B8B8B8"}}),
            url
        )
        .is_err()
    );
    assert!(
        invoke(
            &window,
            "set_caption_mode",
            json!({"request":{"mode":"auto"}}),
            url
        )
        .is_err()
    );
    assert!(
        invoke(
            &window,
            "set_shortcut",
            json!({"request":{"shortcut":"Win+L"}}),
            url
        )
        .is_err()
    );
}

#[test]
fn acl_denies_other_windows_remote_origins_and_ungranted_commands() {
    let app = app();
    let overlay = WebviewWindowBuilder::new(&app, "overlay", WebviewUrl::default())
        .build()
        .unwrap();
    let other = WebviewWindowBuilder::new(&app, "other", WebviewUrl::default())
        .build()
        .unwrap();
    assert!(invoke(&other, "get_state", json!({}), "http://tauri.localhost").is_err());
    assert!(invoke(&overlay, "get_state", json!({}), "https://example.com").is_err());
    assert!(
        invoke(
            &other,
            "fit_settings_window",
            json!({}),
            "http://tauri.localhost"
        )
        .is_err()
    );
    assert!(
        invoke(
            &other,
            "fit_caption_window",
            json!({}),
            "http://tauri.localhost"
        )
        .is_err()
    );
    assert!(
        invoke(
            &overlay,
            "fit_settings_window",
            json!({}),
            "https://example.com"
        )
        .is_err()
    );
    assert!(
        invoke(
            &overlay,
            "plugin:window|set_always_on_top",
            json!({"value":false}),
            "http://tauri.localhost"
        )
        .is_err()
    );
    assert!(
        invoke(
            &overlay,
            "plugin:event|emit",
            json!({"event":"app-state-changed","payload":{}}),
            "http://tauri.localhost"
        )
        .is_err()
    );
}
