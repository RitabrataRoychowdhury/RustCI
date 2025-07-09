use crate::{
    handlers::{
        get_me_handler, github_oauth_callback, github_oauth_handler, google_oauth_handler,
        logout_handler,
    },
    middleware::auth,
    AppState,
};
use axum::{
    extract::{Extension, State},
    middleware,
    routing::get,
    Router,
};
use uuid::Uuid;

pub fn auth_router() -> Router<AppState> {
    Router::new()
        .route("/oauth/google", get(google_oauth_handler))
        .route("/oauth/github", get(github_oauth_handler))
        .route("/oauth/github/callback", get(github_oauth_callback))
        .route("/logout", get(logout_handler))
        .route(
            "/me",
            get(|Extension(user_id): Extension<Uuid>, state: State<AppState>| {
                get_me_handler(user_id, state)
            })
            .route_layer(middleware::from_fn_with_state(
                AppState {
                    env: std::sync::Arc::new(crate::config::Config::init()),
                    db: std::sync::Arc::new(
                        // This is a placeholder - the actual state will be passed from main
                        tokio::runtime::Handle::current().block_on(async {
                            crate::database::DatabaseManager::new("", "").await.unwrap()
                        })
                    ),
                },
                auth,
            )),
        )
}