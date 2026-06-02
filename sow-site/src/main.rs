mod game_manifest;
mod layout;
mod routes;

use axum::{
    body::Body,
    http::{header, Request, Response},
    middleware::{self, Next},
    routing::get,
    Router,
};
use leptos::prelude::*;
use leptos_axum::render_app_to_stream;
use routes::{HomePage, PrivacyPage, TermsPage};
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;

/// Leptos SSR streams start at `<html>`; prepend doctype for standards mode + Lighthouse.
async fn prepend_html_doctype(req: Request<Body>, next: Next) -> Response<Body> {
    let res = next.run(req).await;
    let is_html = res
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.starts_with("text/html"))
        .unwrap_or(false);
    if !is_html {
        return res;
    }

    let (parts, body) = res.into_parts();
    let bytes = match axum::body::to_bytes(body, 2 * 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => return Response::from_parts(parts, Body::empty()),
    };
    let mut out = Vec::with_capacity(16 + bytes.len());
    out.extend_from_slice(b"<!DOCTYPE html>\n");
    out.extend_from_slice(&bytes);
    Response::from_parts(parts, Body::from(out))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sow_site=info,tower_http=info".into()),
        )
        .init();

    let listen = std::env::var("SOW_SITE_LISTEN").unwrap_or_else(|_| "127.0.0.1:8787".into());
    let addr: SocketAddr = listen.parse().expect("SOW_SITE_LISTEN must be host:port");

    let html_routes = Router::new()
        .route("/", get(render_app_to_stream(|| view! { <HomePage/> })))
        .route(
            "/privacy",
            get(render_app_to_stream(|| view! { <PrivacyPage/> })),
        )
        .route(
            "/terms",
            get(render_app_to_stream(|| view! { <TermsPage/> })),
        )
        .layer(middleware::from_fn(prepend_html_doctype));

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .merge(html_routes)
        .layer(TraceLayer::new_for_http());

    tracing::info!("sow-site listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
