use helper_backend::app_core::AppCore;
use tracing::info;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "helper_backend=info".to_string()),
        )
        .init();

    let app = AppCore::default();
    info!(
        budget_used = app.budget.used_cny,
        budget_limit = app.budget.limit_cny,
        "backend core booted"
    );
}
