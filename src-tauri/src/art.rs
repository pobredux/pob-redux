use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use tauri::http::{header, HeaderValue, Request, Response, StatusCode};
use tauri::{Manager, UriSchemeContext, UriSchemeResponder, Wry};

const ART_BASE: &str = "https://art.pobredux.com";
const MAP_REVALIDATE_TIMEOUT: Duration = Duration::from_secs(5);

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(concat!("pob-redux/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(20))
            .build()
            .expect("art http client")
    })
}

fn respond(status: StatusCode, body: Vec<u8>, mime: &str, cache: &str) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::CACHE_CONTROL, cache)
        .body(body)
        .unwrap()
}

fn not_found() -> Response<Vec<u8>> {
    respond(StatusCode::NOT_FOUND, Vec::new(), "text/plain", "no-store")
}

async fn download(segments: &[&str], version: Option<&str>) -> Option<Vec<u8>> {
    let mut url = reqwest::Url::parse(ART_BASE).ok()?;
    url.path_segments_mut().ok()?.extend(segments);
    if let Some(v) = version {
        url.query_pairs_mut().append_pair("v", v);
    }
    let res = client().get(url).send().await.ok()?;
    if !res.status().is_success() {
        return None;
    }
    res.bytes().await.ok().map(|b| b.to_vec())
}

pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> bool {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension(format!("tmp{}", NEXT.fetch_add(1, Ordering::Relaxed)));
    if std::fs::write(&tmp, bytes).is_ok() && std::fs::rename(&tmp, path).is_ok() {
        return true;
    }
    let _ = std::fs::remove_file(&tmp);
    false
}

async fn map(cache: &Path, game: &str) -> Response<Vec<u8>> {
    let file = cache.join("maps").join(format!("{game}.json"));
    let etag_file = file.with_extension("etag");
    let mut req = client().get(format!("{ART_BASE}/maps/{game}/latest.json"));
    if file.is_file() {
        req = req.timeout(MAP_REVALIDATE_TIMEOUT);
        let etag = std::fs::read_to_string(&etag_file).ok();
        if let Some(value) = etag.and_then(|e| HeaderValue::from_str(e.trim()).ok()) {
            req = req.header(header::IF_NONE_MATCH, value);
        }
    }
    if let Ok(res) = req.send().await {
        if res.status().is_success() {
            let etag = res.headers().get(header::ETAG).cloned();
            if let Ok(bytes) = res.bytes().await {
                if serde_json::from_slice::<serde_json::Value>(&bytes).is_ok() && write_atomic(&file, &bytes) {
                    match etag {
                        Some(etag) => {
                            write_atomic(&etag_file, etag.as_bytes());
                        }
                        None => {
                            let _ = std::fs::remove_file(&etag_file);
                        }
                    }
                }
            }
        }
    }
    match std::fs::read(&file) {
        Ok(bytes) => respond(StatusCode::OK, bytes, "application/json", "no-cache"),
        Err(_) => not_found(),
    }
}

async fn image(cache: &Path, game: &str, tag: &str, path: &str) -> Response<Vec<u8>> {
    let Some(stem) = path.strip_suffix(".webp") else {
        return not_found();
    };
    let file = cache.join(game).join(format!("{stem}.{tag}.webp"));
    let bytes = match std::fs::read(&file) {
        Ok(bytes) => bytes,
        Err(_) => {
            let segments: Vec<&str> = std::iter::once(game).chain(path.split('/')).collect();
            let Some(bytes) = download(&segments, Some(tag)).await else {
                return not_found();
            };
            write_atomic(&file, &bytes);
            bytes
        }
    };
    respond(StatusCode::OK, bytes, "image/webp", "max-age=31536000, immutable")
}

fn valid_game(game: &str) -> bool {
    game == "poe1" || game == "poe2"
}

fn valid_tag(tag: &str) -> bool {
    tag.len() == 8 && tag.bytes().all(|b| b.is_ascii_hexdigit())
}

fn valid_path(path: &str) -> bool {
    path.starts_with("Art/") && path.split('/').all(|part| !part.is_empty() && part != "..") && !path.contains(['\\', ':'])
}

async fn route(cache: &Path, rel: &str) -> Response<Vec<u8>> {
    match rel.splitn(3, '/').collect::<Vec<_>>().as_slice() {
        ["maps", file] => match file.strip_suffix(".json") {
            Some(game) if valid_game(game) => map(cache, game).await,
            _ => not_found(),
        },
        [game, tag, path] if valid_game(game) && valid_tag(tag) && valid_path(path) => image(cache, game, tag, path).await,
        _ => not_found(),
    }
}

/// `pobart://localhost/maps/<game>.json` and `/<game>/<tag>/<Art path>`, cached from art.pobredux.com.
pub fn serve(ctx: UriSchemeContext<'_, Wry>, request: Request<Vec<u8>>, responder: UriSchemeResponder) {
    let cache = ctx.app_handle().path().app_cache_dir().ok().map(|dir| dir.join("art"));
    let rel = crate::percent_decode(request.uri().path().trim_start_matches('/'));
    tauri::async_runtime::spawn(async move {
        let response = match cache {
            Some(cache) => route(&cache, &rel).await,
            None => not_found(),
        };
        responder.respond(response);
    });
}
