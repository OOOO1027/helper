use helper_backend::app_core::AppCore;
use helper_backend::ipc::commands::tauri_api;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY};
use tauri::webview::InvokeRequest;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn invoke_json(
    webview: &tauri::WebviewWindow<tauri::test::MockRuntime>,
    cmd: &str,
    body: Value,
) -> Value {
    let response = get_ipc_response(
        webview,
        InvokeRequest {
            cmd: cmd.into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "http://tauri.localhost".parse().expect("url"),
            body: tauri::ipc::InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .expect("ipc response");
    response
        .deserialize::<Value>()
        .expect("deserialize ipc json")
}

fn build_webview() -> (
    tauri::App<tauri::test::MockRuntime>,
    tauri::WebviewWindow<tauri::test::MockRuntime>,
) {
    let state = Arc::new(Mutex::new(AppCore::default()));
    let app = mock_builder()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            tauri_api::get_local_tool_status,
            tauri_api::run_notion_sync_once,
            tauri_api::retry_dead_letters,
            tauri_api::import_wechat_and_persist,
            tauri_api::get_sync_daily_stats
        ])
        .build(mock_context(noop_assets()))
        .expect("build mock app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build webview");
    (app, webview)
}

#[test]
fn invoke_surface_for_four_required_commands() {
    let _guard = env_lock().lock().expect("env lock");
    std::env::remove_var("HELPER_DB_PATH");
    std::env::remove_var("HELPER_DB_KEY");
    std::env::remove_var("NOTION_TOKEN");
    std::env::remove_var("NOTION_DATABASE_ID");

    let (_app, webview) = build_webview();

    let tool_status = invoke_json(&webview, "get_local_tool_status", json!({}));
    println!("invoke:get_local_tool_status => {}", tool_status);
    assert_eq!(tool_status["ok"], json!(true));
    assert!(tool_status["data"]["textutil"].is_boolean());
    assert!(tool_status["data"]["pdftotext"].is_boolean());

    let notion_sync = invoke_json(&webview, "run_notion_sync_once", json!({ "req": { "limit": 1 } }));
    println!("invoke:run_notion_sync_once => {}", notion_sync);
    assert_eq!(notion_sync["ok"], json!(false));
    assert_eq!(notion_sync["error"]["code"], json!("IPC-6001"));

    let retry_dlq = invoke_json(
        &webview,
        "retry_dead_letters",
        json!({ "req": { "ids": ["dlq_sync_x", "dlq_pipe_x"] } }),
    );
    println!("invoke:retry_dead_letters => {}", retry_dlq);
    assert_eq!(retry_dlq["ok"], json!(false));
    assert_eq!(retry_dlq["error"]["code"], json!("IPC-6001"));

    let import_persist = invoke_json(
        &webview,
        "import_wechat_and_persist",
        json!({ "req": { "paths": ["/tmp/a.txt"] } }),
    );
    println!("invoke:import_wechat_and_persist => {}", import_persist);
    assert_eq!(import_persist["ok"], json!(false));
    assert_eq!(import_persist["error"]["code"], json!("IPC-6001"));

    let daily_stats = invoke_json(
        &webview,
        "get_sync_daily_stats",
        json!({ "req": { "range": { "from": "2026-02-01 00:00:00", "to": "2026-02-28 23:59:59" }, "page": { "page": 1, "page_size": 20 } } }),
    );
    println!("invoke:get_sync_daily_stats => {}", daily_stats);
    assert_eq!(daily_stats["ok"], json!(false));
    assert_eq!(daily_stats["error"]["code"], json!("IPC-6001"));
}
