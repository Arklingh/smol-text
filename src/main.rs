// to be removed
#![allow(unused_imports)]
use axum::{
    Form, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
};
use nanoid::nanoid;
use pulldown_cmark::{Options, Parser, html};
use serde::Deserialize;
use sqlx::PgPool;
use std::net::SocketAddr;
use std::{env, fs::create_dir};
use tower_http::services::{ServeDir, ServeFile};

// Axum app state
// db - db connections pool
#[derive(Clone)]
struct AppState {
    db: PgPool,
}

#[derive(Deserialize)]
struct CreateText {
    title: String,
    content: String,
}

#[tokio::main]
async fn main() {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let port = env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()
        .expect("PORT must be set");

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    let state = AppState { db: pool };

    let app = Router::new()
        .route("/test", get(|| async { "Axum is working!" }))
        .route("/api/paste", post(create_text))
        .route("/api/paste/{slug}/share", post(increment_share))
        .route("/p/{slug}", get(get_text))
        .route("/p/{slug}/analytics", get(get_analytics))
        .fallback_service(ServeDir::new("static").fallback(ServeFile::new("static/index.html")))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("Server started at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn create_text(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(input): Form<CreateText>,
) -> impl IntoResponse {
    let alphabet: [char; 62] = [
        'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r',
        's', 't', 'u', 'v', 'w', 'x', 'y', 'z', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J',
        'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '0', '1',
        '2', '3', '4', '5', '6', '7', '8', '9',
    ];
    let slug = nanoid!(7, &alphabet);

    let query = "INSERT INTO texts (slug, title, content) VALUES ($1, $2, $3)";
    let result = sqlx::query(query)
        .bind(&slug)
        .bind(&input.title)
        .bind(&input.content)
        .execute(&state.db)
        .await;

    match result {
        Ok(_) => {
            let host = headers
                .get("host")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("localhost:8080");

            let protocol = if host.contains("localhost") || host.contains("127.0.0.1") {
                "http"
            } else {
                "https"
            };

            let full_article_url = format!("{}://{}/p/{}", protocol, host, slug);

            let success_html = format!(
                r#"<!DOCTYPE html>
                <html lang="en">
                <head>
                    <meta charset="UTF-8">
                    <title>Published! — Smol Text</title>
                    <link href="https://cdn.jsdelivr.net/npm/tailwindcss@2.2.19/dist/tailwind.min.css" rel="stylesheet">
                    <style>body {{ font-family: ui-sans-serif, system-ui, sans-serif; background-color: #f9fafb; }}</style>
                    <script>
                        function copyAndShare(slug) {{
                            navigator.clipboard.writeText(window.location.origin + '/p/' + slug);
                            
                            fetch('/api/paste/' + slug + '/share', {{ method: 'POST' }})
                            .then(response => {{
                                if (!response.ok) console.error('Server error updating shares counter');
                            }})
                            .catch(err => console.error('Network error:', err));
                            
                            alert('Link copied to clipboard!');
                        }}
                    </script>
                </head>
                <body class="h-screen flex items-center justify-center">
                    <div class="bg-white p-8 rounded-xl shadow-sm border border-gray-100 max-w-md w-full text-center">
                        <div class="text-green-500 text-5xl mb-4">✓</div>
                        <h1 class="text-2xl font-bold text-gray-900 mb-2">Published!</h1>
                        <p class="text-gray-500 mb-6">Your text is ready on the link below.</p>
                        
                        <div class="flex items-center space-x-2 bg-gray-50 p-3 rounded-lg border border-gray-200 mb-6">
                            <input id="link" type="text" readonly value="/p/{}" class="bg-transparent w-full text-sm font-mono text-gray-700 outline-none">
                            <button onclick="copyAndShare('{}')" class="text-xs bg-blue-600 text-white px-3 py-1.5 rounded hover:bg-blue-700 transition">Copy</button>
                        </div>

                        <div class="flex justify-center mb-6 bg-gray-50 p-4 rounded-lg inline-block">
                            <img src="https://api.qrserver.com/v1/create-qr-code/?size=150x150&data={}" alt="QR Code" class="w-32 h-32">
                        </div>

                        <div class="grid grid-cols-3 gap-2 text-xs">
                            <a href="/p/{}" class="block bg-gray-900 text-white py-2 rounded-lg hover:bg-gray-800 transition">Read text</a>
                            <a href="/p/{}/analytics" class="block bg-gray-100 text-gray-700 py-2 rounded-lg hover:bg-gray-200 transition">Analytics</a>
                            <a href="/" class="block bg-blue-50 text-blue-600 py-2 rounded-lg hover:bg-blue-100 transition font-medium">New text</a>
                        </div>
                    </div>
                </body>
                </html>"#,
                slug,
                slug,
                urlencoding::encode(&full_article_url),
                slug,
                slug
            );
            Html(success_html).into_response()
        }
        Err(e) => {
            eprintln!("Database error: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("DB error: {}", e),
            )
                .into_response()
        }
    }
}

async fn get_text(Path(slug): Path<String>, State(state): State<AppState>) -> impl IntoResponse {
    let update_result = sqlx::query("UPDATE texts SET views = views + 1 WHERE slug = $1")
        .bind(&slug)
        .execute(&state.db)
        .await;

    if let Err(e) = update_result {
        eprintln!("Failed to update views for slug {}: {:?}", slug, e);
    }

    let query = "SELECT title, content FROM texts WHERE slug = $1";
    let row = sqlx::query_as::<_, (String, String)>(query)
        .bind(&slug)
        .fetch_optional(&state.db)
        .await;

    match row {
        Ok(Some((title, content))) => {
            let mut options = Options::empty();
            options.insert(Options::ENABLE_TABLES);
            options.insert(Options::ENABLE_STRIKETHROUGH);

            let parser = Parser::new_ext(&content, options);
            let mut html_output = String::new();
            html::push_html(&mut html_output, parser);

            let full_html = format!(
                r#"<!DOCTYPE html>
            <html lang="en">
            <head>
                <meta charset="UTF-8">
                <title>{} — Smol Text</title>
                <link href="https://fonts.googleapis.com/css2?family=Source+Serif+Pro:ital,wght@0,400;0,600;1,400&display=swap" rel="stylesheet">
                <link href="https://cdn.jsdelivr.net/npm/tailwindcss@2.2.19/dist/tailwind.min.css" rel="stylesheet">
                <style>
                    body {{ background-color: #fbfbfb; color: #111111; }}
                    .font-serif {{ font-family: 'Source Serif Pro', Georgia, serif; }}
                    .prose p {{ margin-bottom: 1.75rem; font-size: 1.25rem; line-height: 1.8; }}
                    .prose h2 {{ font-size: 1.75rem; font-weight: 600; margin-top: 2rem; margin-bottom: 1rem; }}
                    .prose blockquote {{ border-left: 3px solid #111; padding-left: 1.5rem; font-style: italic; margin: 2rem 0; }}
                </style>
            </head>
            <body>
                <div class="max-w-3xl mx-auto px-6 pt-8 flex justify-between items-center text-xs text-gray-400 font-sans">
                    <span class="font-bold tracking-wider text-gray-800">SMOL TEXT</span>
                    <button onclick="shareArticle('{}')" class="hover:text-blue-600 font-medium transition">Share</button>
                </div>

                <article class="max-w-2xl mx-auto px-6 py-16 font-serif">
                    <h1 class="text-4xl md:text-5xl font-bold tracking-tight mb-10 text-gray-900 leading-tight">{}</h1>
                    <div class="prose">{}</div>
                </article>

                <script>
                    function shareArticle(slug) {{
                        fetch('/api/paste/' + slug + '/share', {{ method: 'POST' }})
                        .then(response => {{
                            if (response.ok) {{
                                navigator.clipboard.writeText(window.location.href);
                                alert('Link copied to clipboard! Analytics counter updated.');
                            }} else {{
                                console.error('Server returned an error:', response.status);
                            }}
                        }})
                        .catch(err => console.error('Network error:', err));
                    }}
                </script>
            </body>
            </html>"#,
                title, slug, title, html_output
            );

            Html(full_html).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Text not found").into_response(),
        Err(e) => {
            eprintln!("DB error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Error during search").into_response()
        }
    }
}

async fn get_analytics(
    Path(slug): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let query = "SELECT title, views, shares FROM texts WHERE slug = $1";

    let row = sqlx::query_as::<_, (String, i32, i32)>(query)
        .bind(&slug)
        .fetch_optional(&state.db)
        .await;

    match row {
        Ok(Some((title, views, shares))) => {
            let analytics_html = format!(
                r#"<!DOCTYPE html>
                <html lang="en">
                <head>
                    <meta charset="UTF-8">
                    <title>Analytics: {} — Smol Text</title>
                    <link href="https://cdn.jsdelivr.net/npm/tailwindcss@2.2.19/dist/tailwind.min.css" rel="stylesheet">
                    <style>body {{ font-family: ui-sans-serif, system-ui, sans-serif; background-color: #f9fafb; }}</style>
                </head>
                <body class="p-6 md:p-12">
                    <div class="max-w-3xl mx-auto">
                        <div class="mb-8 flex items-center justify-between">
                            <div>
                                <a href="/" class="text-sm text-blue-600 hover:underline">← Back to editor</a>
                                <h1 class="text-2xl font-bold text-gray-900 mt-2">Post Analytics</h1>
                                <p class="text-gray-500 font-serif text-lg mt-1">"{}"</p>
                            </div>
                        </div>

                        <div class="grid grid-cols-1 md:grid-cols-2 gap-6 mb-8">
                            <div class="bg-white p-6 rounded-xl border border-gray-100 shadow-sm">
                                <p class="text-sm font-medium text-gray-400 uppercase tracking-wider">Total Views</p>
                                <p class="text-4xl font-bold text-gray-900 mt-2">{}</p>
                            </div>
                            <div class="bg-white p-6 rounded-xl border border-gray-100 shadow-sm">
                                <p class="text-sm font-medium text-gray-400 uppercase tracking-wider">Total Shares</p>
                                <p class="text-4xl font-bold text-gray-900 mt-2">{}</p>
                            </div>
                        </div>
                    </div>
                </body>
                </html>"#,
                title, title, views, shares
            );
            Html(analytics_html).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Record not found").into_response(),
        Err(e) => {
            eprintln!("Analytics DB error: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Server Error").into_response()
        }
    }
}

async fn increment_share(Path(slug): Path<String>, State(state): State<AppState>) -> StatusCode {
    let query = "UPDATE texts SET shares = shares + 1 WHERE slug = $1";
    match sqlx::query(query).bind(&slug).execute(&state.db).await {
        Ok(result) => {
            if result.rows_affected() == 0 {
                eprintln!(
                    "Warning: No rows updated for share execution on slug: {}",
                    slug
                );
            }
            StatusCode::OK
        }
        Err(e) => {
            eprintln!("Failed to update share record in DB: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
