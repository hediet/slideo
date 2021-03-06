use crate::db::{DbPool, PdfVideoMatching};
use actix_cors::Cors;
use actix_files::NamedFile;
use actix_web::{
    body::Body,
    get,
    web::{self, Json},
    App, HttpServer,
};
use anyhow::{anyhow, Result};
use rust_embed::RustEmbed;
use std::borrow::Cow;

struct AppState {
    db_pool: DbPool,
}

#[derive(Debug)]
struct AnyHowErrorAdapter {
    err: anyhow::Error,
}

impl std::fmt::Display for AnyHowErrorAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.err.fmt(f)
    }
}

impl actix_web::error::ResponseError for AnyHowErrorAdapter {}
impl From<anyhow::Error> for AnyHowErrorAdapter {
    fn from(err: anyhow::Error) -> Self {
        Self { err }
    }
}

impl From<std::io::Error> for AnyHowErrorAdapter {
    fn from(err: std::io::Error) -> Self {
        Self { err: err.into() }
    }
}

#[get("/pdf-matchings/{hash}")]
async fn pdf_matches_handler(
    web::Path(pdf_hash): web::Path<String>,
    data: web::Data<AppState>,
) -> actix_web::Result<Json<Vec<PdfVideoMatching>>, AnyHowErrorAdapter> {
    let mut db = data.db_pool.db().await?;

    let result = db.get_pdf_video_matchings(&pdf_hash).await?;

    Ok(Json(result))
}

#[get("/files/{hash}")]
async fn files_handler(
    web::Path(hash): web::Path<String>,
    data: web::Data<AppState>,
) -> actix_web::Result<NamedFile, AnyHowErrorAdapter> {
    let mut db = data.db_pool.db().await?;
    let path = db.get_path(&hash).await?;

    if let Some(path) = path {
        Ok(NamedFile::open(path)?)
    } else {
        Err(anyhow!("Hash not known"))?
    }
}

#[derive(RustEmbed)]
#[folder = "../../webview/dist"]
struct Dist;

fn handle_embedded_file(path: &str) -> web::HttpResponse {
    match Dist::get(path) {
        Some(content) => {
            let body: Body = match content {
                Cow::Borrowed(bytes) => bytes.into(),
                Cow::Owned(bytes) => bytes.into(),
            };
            web::HttpResponse::Ok()
                .content_type(mime_guess::from_path(path).first_or_octet_stream().as_ref())
                .body(body)
        }
        None => web::HttpResponse::NotFound().body("404 Not Found"),
    }
}

#[get("/")]
fn index() -> web::HttpResponse {
    handle_embedded_file("index.html")
}

#[get("{_:.*}")]
fn dist(path: web::Path<String>) -> web::HttpResponse {
    handle_embedded_file(&path.0)
}

#[actix_web::main]
pub async fn start_server(pdf_hash: Option<String>) -> Result<()> {
    let db_pool = DbPool::connect().await?;

    if let Some(pdf_hash) = pdf_hash {
        println!("View pdf on http://localhost:63944/?pdf-hash={}", pdf_hash);
    } else {
        println!("Server is running on http://localhost:63944");
    }

    HttpServer::new(move || {
        App::new()
            .wrap(Cors::default().allowed_origin("http://127.0.0.1:8080"))
            .data(AppState {
                db_pool: db_pool.clone(),
            })
            .service(files_handler)
            .service(pdf_matches_handler)
            .service(index)
            .service(dist)
    })
    .bind("127.0.0.1:63944")?
    .run()
    .await?;

    Ok(())
}
