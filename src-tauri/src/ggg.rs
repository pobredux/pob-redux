//! Sign-in to pathofexile.com with OAuth, and the account API calls that use the token.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine;
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, State, Url};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};

use crate::agent::random_hex;
use crate::character::query_value;
use crate::game::Game;
use crate::AppState;

const CLIENT_ID: &str = "pobredux";
const SCOPES: &str = "account:profile account:characters account:trade";
/// GGG registered these loopback ports for the client; no other redirect is accepted.
const PORTS: [u16; 3] = [49095, 49096, 49097];
const AUTHORIZE: &str = "https://www.pathofexile.com/oauth/authorize";
const TOKEN: &str = "https://www.pathofexile.com/oauth/token";
const API: &str = "https://api.pathofexile.com";
/// GGG's API rules fix this format for every OAuth client.
const UA: &str = concat!("OAuth pobredux/", env!("CARGO_PKG_VERSION"), " (contact: contact@pobredux.com)");
const SECRET: &str = "ggg-oauth";
const SIGN_IN_WAIT: Duration = Duration::from_secs(300);
/// GGG ends a public client's sign-in seven days after the consent, however often the token is renewed.
const SIGN_IN_LIFETIME: u64 = 7 * 24 * 60 * 60;
const SIGNED_OUT: &str = "Sign in with Path of Exile first.";
const EXPIRED: &str = "The Path of Exile sign-in has expired. Sign in again.";
const NEEDS_TRADE: &str = "Sign in with Path of Exile again to allow trade searches.";
/// A trade request may only go to these addresses, so nothing else can be sent the token.
const TRADE_API: [&str; 6] = [
    "https://www.pathofexile.com/api/trade2/search/",
    "https://www.pathofexile.com/api/trade2/fetch/",
    "https://www.pathofexile.com/api/trade/search/",
    "https://www.pathofexile.com/api/trade/fetch/",
    "https://www.pathofexile.com/api/trade2/data/leagues",
    "https://www.pathofexile.com/api/trade/data/leagues",
];

#[derive(Serialize, Deserialize, Clone)]
struct Session {
    account: String,
    access: String,
    refresh: Option<String>,
    expires_at: u64,
    ends_at: u64,
    #[serde(default)]
    scope: String,
}

impl Session {
    /// `earlier` is the session a refresh renews; without one this is a new sign-in.
    fn new(tokens: TokenResponse, earlier: Option<Session>) -> Self {
        let (account, scope, ends_at) = match earlier {
            Some(session) => (session.account, session.scope, session.ends_at),
            None => (String::new(), String::new(), now() + SIGN_IN_LIFETIME),
        };
        Session {
            account: tokens.username.unwrap_or(account),
            access: tokens.access_token,
            refresh: tokens.refresh_token,
            expires_at: now() + tokens.expires_in,
            ends_at,
            scope: tokens.scope.unwrap_or(scope),
        }
    }

    fn allows_trade(&self) -> bool {
        self.scope.split_whitespace().any(|scope| scope == "account:trade")
    }
}

#[derive(Default)]
struct Store {
    loaded: bool,
    session: Option<Session>,
}

impl Store {
    fn set(&mut self, app: &AppHandle, session: Session) {
        match serde_json::to_string(&session).map_err(|e| e.to_string()).and_then(|json| crate::ai::store_secret(app, SECRET, &json)) {
            Ok(()) => {}
            Err(e) => log::warn!("ggg: the sign-in will not outlive this run: {e}"),
        }
        self.session = Some(session);
    }

    fn clear(&mut self, app: &AppHandle) {
        self.session = None;
        if let Err(e) = crate::ai::clear_secret(app, SECRET) {
            log::warn!("ggg: could not remove the saved sign-in: {e}");
        }
    }
}

#[derive(Default)]
pub struct GggState {
    store: tokio::sync::Mutex<Store>,
    pending: Mutex<Option<oneshot::Sender<()>>>,
    not_before: Mutex<HashMap<String, Instant>>,
}

impl GggState {
    async fn store(&self, app: &AppHandle) -> tokio::sync::MutexGuard<'_, Store> {
        let mut store = self.store.lock().await;
        if !store.loaded {
            store.session = saved(app);
            store.loaded = true;
        }
        store
    }

    fn wait(&self, policy: &str) -> Option<u64> {
        let until = *self.not_before.lock().unwrap().get(policy)?;
        Some(until.checked_duration_since(Instant::now())?.as_secs() + 1)
    }

    fn limit(&self, policy: &str, wait: u64) {
        let mut not_before = self.not_before.lock().unwrap();
        if wait == 0 {
            not_before.remove(policy);
        } else {
            not_before.insert(policy.to_string(), Instant::now() + Duration::from_secs(wait));
        }
    }
}

fn saved(app: &AppHandle) -> Option<Session> {
    serde_json::from_str(&crate::ai::stored_key(app, SECRET)?).ok()
}

pub(crate) fn secrets(app: &AppHandle) -> Vec<String> {
    saved(app).map(|s| std::iter::once(s.access).chain(s.refresh).collect()).unwrap_or_default()
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder().user_agent(UA).timeout(Duration::from_secs(20)).build().map_err(|e| e.to_string())
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
    refresh_token: Option<String>,
    username: Option<String>,
    scope: Option<String>,
}

enum TokenError {
    Rejected(String),
    Failed(String),
}

async fn request_token(form: String) -> Result<TokenResponse, TokenError> {
    let failed = |e: reqwest::Error| TokenError::Failed(format!("pathofexile.com: {e}"));
    let resp = client()
        .map_err(TokenError::Failed)?
        .post(TOKEN)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(form)
        .send()
        .await
        .map_err(failed)?;
    let status = resp.status().as_u16();
    let body = resp.text().await.map_err(failed)?;
    match status {
        200..=299 => serde_json::from_str(&body).map_err(|_| TokenError::Failed("pathofexile.com sent a sign-in reply this app could not read".into())),
        400 | 401 => {
            let reply: Value = serde_json::from_str(&body).unwrap_or_default();
            let why = ["error_description", "error"].iter().find_map(|k| reply.get(k)?.as_str()).unwrap_or("no reason given");
            Err(TokenError::Rejected(why.to_string()))
        }
        _ => Err(TokenError::Failed(format!("pathofexile.com returned HTTP {status}"))),
    }
}

/// `stale` is a token the API just refused, which forces a refresh unless another call already made one.
async fn access_token(app: &AppHandle, state: &GggState, stale: Option<&str>) -> Result<String, String> {
    let mut store = state.store(app).await;
    let session = store.session.clone().ok_or(SIGNED_OUT)?;
    if session.expires_at > now() + 60 && stale != Some(session.access.as_str()) {
        return Ok(session.access);
    }
    let Some(refresh) = session.refresh.clone() else {
        store.clear(app);
        return Err(EXPIRED.into());
    };
    match request_token(format!("client_id={CLIENT_ID}&grant_type=refresh_token&refresh_token={}", query_value(&refresh))).await {
        Ok(tokens) => {
            let access = tokens.access_token.clone();
            store.set(app, Session::new(tokens, Some(session)));
            Ok(access)
        }
        Err(TokenError::Rejected(why)) => {
            log::info!("ggg: pathofexile.com would not renew the sign-in: {why}");
            store.clear(app);
            Err(EXPIRED.into())
        }
        Err(TokenError::Failed(e)) => Err(e),
    }
}

fn triple(s: &str) -> Option<[u64; 3]> {
    let mut parts = s.trim().split(':').map(|n| n.parse::<u64>().ok());
    Some([parts.next()??, parts.next()??, parts.next()??])
}

/// Seconds to hold off, from GGG's rate limit headers: a rule at its limit waits out its period.
fn wait_from(headers: &HeaderMap) -> u64 {
    let text = |name: &str| headers.get(name).and_then(|v| v.to_str().ok()).unwrap_or_default();
    let mut wait = text("retry-after").trim().parse().unwrap_or(0);
    for rule in text("x-rate-limit-rules").split(',').map(|r| r.trim().to_ascii_lowercase()).filter(|r| !r.is_empty()) {
        let limits = text(&format!("x-rate-limit-{rule}")).split(',');
        let states = text(&format!("x-rate-limit-{rule}-state")).split(',');
        for (limit, state) in limits.zip(states) {
            let (Some([max, period, _]), Some([hits, _, restricted])) = (triple(limit), triple(state)) else { continue };
            if restricted > 0 {
                wait = wait.max(restricted);
            } else if hits >= max {
                wait = wait.max(period);
            }
        }
    }
    wait
}

fn limited(seconds: u64) -> String {
    let unit = if seconds == 1 { "second" } else { "seconds" };
    format!("pathofexile.com is limiting requests. Try again in {seconds} {unit}.")
}

struct Reply {
    status: u16,
    wait: u64,
    body: String,
}

async fn send(app: &AppHandle, state: &GggState, policy: &str, request: impl Fn(&reqwest::Client) -> reqwest::RequestBuilder) -> Result<Reply, String> {
    let client = client()?;
    let mut token = access_token(app, state, None).await?;
    let mut renewed = false;
    loop {
        let resp = request(&client).bearer_auth(&token).send().await.map_err(|e| format!("pathofexile.com: {e}"))?;
        let wait = wait_from(resp.headers());
        state.limit(policy, wait);
        let status = resp.status().as_u16();
        if status == 401 {
            if !renewed {
                renewed = true;
                token = access_token(app, state, Some(&token)).await?;
                continue;
            }
            state.store(app).await.clear(app);
            return Err(EXPIRED.into());
        }
        let body = resp.text().await.map_err(|e| format!("pathofexile.com: {e}"))?;
        return Ok(Reply { status, wait, body });
    }
}

async fn api_get(app: &AppHandle, state: &GggState, policy: &str, path: &str) -> Result<Value, String> {
    if let Some(wait) = state.wait(policy) {
        return Err(limited(wait));
    }
    let reply = send(app, state, policy, |client| client.get(format!("{API}{path}"))).await?;
    match reply.status {
        200..=299 => serde_json::from_str(&reply.body).map_err(|_| "pathofexile.com sent a reply this app could not read".to_string()),
        403 => Err("pathofexile.com refused the request. Sign out, then sign in again.".into()),
        404 => Err("pathofexile.com has no such character.".into()),
        429 => Err(limited(reply.wait.max(1))),
        status => Err(format!("pathofexile.com returned HTTP {status}")),
    }
}

async fn bind() -> Result<(TcpListener, u16), String> {
    for port in PORTS {
        if let Ok(listener) = TcpListener::bind(("127.0.0.1", port)).await {
            return Ok((listener, port));
        }
    }
    Err(format!("The sign-in needs one of the ports {} to {} on this computer, and all three are in use.", PORTS[0], PORTS[2]))
}

/// `None` for a request that is not this sign-in's redirect, so a stray local request cannot end it.
fn callback(target: &str, csrf: &str) -> Option<Result<String, String>> {
    let url = Url::parse(&format!("http://127.0.0.1{target}")).ok()?;
    if url.path() != "/callback" {
        return None;
    }
    let param = |key: &str| url.query_pairs().find(|(k, _)| k == key).map(|(_, v)| v.into_owned());
    if param("state").as_deref() != Some(csrf) {
        return None;
    }
    if let Some(code) = param("code") {
        return Some(Ok(code));
    }
    let why = param("error_description").or_else(|| param("error")).unwrap_or_else(|| "no code came back".into());
    Some(Err(format!("pathofexile.com did not sign you in: {why}")))
}

async fn answer(mut stream: TcpStream, csrf: &str) -> Option<Result<String, String>> {
    let mut buf = vec![0u8; 16 * 1024];
    let mut len = 0;
    while !buf[..len].windows(4).any(|w| w == b"\r\n\r\n") {
        if len == buf.len() {
            return None;
        }
        match tokio::time::timeout(Duration::from_secs(10), stream.read(&mut buf[len..])).await {
            Ok(Ok(n)) if n > 0 => len += n,
            _ => return None,
        }
    }
    let request = std::str::from_utf8(&buf[..len]).ok()?;
    let target = request.strip_prefix("GET ")?.split(' ').next()?;
    let outcome = callback(target, csrf);
    let (status, text) = match &outcome {
        Some(Ok(_)) => ("200 OK", "Signed in. You can close this tab and go back to PoB Redux."),
        Some(Err(_)) => ("200 OK", "The sign-in did not finish. Go back to PoB Redux and try again."),
        None => ("404 Not Found", "Not found."),
    };
    let body = format!(
        "<!doctype html><meta charset=\"utf-8\"><title>PoB Redux</title>\
         <body style=\"margin:0;height:100vh;display:grid;place-items:center;background:#111;color:#ddd;font:15px system-ui,sans-serif\"><p>{text}</p>"
    );
    let reply = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(reply.as_bytes()).await;
    let _ = stream.shutdown().await;
    outcome
}

/// Browsers open spare connections that never send a request, so each connection is read on its own task.
async fn wait_for_code(listener: TcpListener, csrf: String) -> Result<String, String> {
    let (tx, mut rx) = mpsc::channel(1);
    loop {
        tokio::select! {
            conn = listener.accept() => {
                let (stream, _) = conn.map_err(|e| format!("the sign-in could not listen for the browser: {e}"))?;
                let (tx, csrf) = (tx.clone(), csrf.clone());
                tauri::async_runtime::spawn(async move {
                    if let Some(outcome) = answer(stream, &csrf).await {
                        let _ = tx.send(outcome).await;
                    }
                });
            }
            Some(outcome) = rx.recv() => return outcome,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    name: String,
    ends_at: u64,
    trade: bool,
}

impl From<&Session> for Account {
    fn from(session: &Session) -> Self {
        Account { name: session.account.clone(), ends_at: session.ends_at, trade: session.allows_trade() }
    }
}

#[tauri::command]
pub async fn ggg_account(app: AppHandle, state: State<'_, GggState>) -> Result<Option<Account>, String> {
    Ok(state.store(&app).await.session.as_ref().map(Account::from))
}

/// `None` when the user cancelled, or started another sign-in that replaced this one.
#[tauri::command]
pub async fn ggg_sign_in(app: AppHandle, state: State<'_, GggState>) -> Result<Option<Account>, String> {
    let (cancel, cancelled) = oneshot::channel();
    *state.pending.lock().unwrap() = Some(cancel);
    let (listener, port) = bind().await?;
    let redirect = format!("http://127.0.0.1:{port}/callback");
    let verifier = random_hex(32)?;
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let csrf = random_hex(16)?;
    let url = format!(
        "{AUTHORIZE}?client_id={CLIENT_ID}&response_type=code&scope={}&state={csrf}&redirect_uri={}&code_challenge={challenge}&code_challenge_method=S256",
        query_value(SCOPES),
        query_value(&redirect)
    );
    let _ = app.emit_to("main", "ggg:sign-in", &url);
    if let Err(e) = crate::browser::open(&app, &url) {
        log::warn!("ggg: could not open the browser: {e}");
    }
    let code = tokio::select! {
        code = tokio::time::timeout(SIGN_IN_WAIT, wait_for_code(listener, csrf)) => {
            code.map_err(|_| "The sign-in was not finished in time. Try again.".to_string())??
        }
        _ = cancelled => return Ok(None),
    };
    let form = format!(
        "client_id={CLIENT_ID}&grant_type=authorization_code&code={}&redirect_uri={}&scope={}&code_verifier={verifier}",
        query_value(&code),
        query_value(&redirect),
        query_value(SCOPES)
    );
    let tokens = request_token(form).await.map_err(|e| match e {
        TokenError::Rejected(why) => format!("pathofexile.com refused the sign-in: {why}"),
        TokenError::Failed(e) => e,
    })?;
    let session = Session::new(tokens, None);
    let account = Account::from(&session);
    state.store(&app).await.set(&app, session);
    Ok(Some(account))
}

#[tauri::command]
pub fn ggg_sign_in_cancel(state: State<'_, GggState>) {
    if let Some(cancel) = state.pending.lock().unwrap().take() {
        let _ = cancel.send(());
    }
}

#[tauri::command]
pub async fn ggg_sign_out(app: AppHandle, state: State<'_, GggState>) -> Result<(), String> {
    state.store(&app).await.clear(&app);
    Ok(())
}

fn realm_path(game: Game, realm: &str) -> Result<&'static str, String> {
    if game == Game::Poe2 {
        return Ok("/poe2");
    }
    match realm.trim().to_ascii_lowercase().as_str() {
        "" | "pc" => Ok(""),
        "xbox" => Ok("/xbox"),
        "sony" => Ok("/sony"),
        other => Err(format!("unknown realm {other:?}")),
    }
}

#[tauri::command]
pub async fn ggg_characters(app: AppHandle, state: State<'_, GggState>, game: State<'_, AppState>, realm: String) -> Result<Vec<Value>, String> {
    let realm = realm_path(game.game(), &realm)?;
    let mut reply = api_get(&app, &state, &format!("list{realm}"), &format!("/character{realm}")).await?;
    match reply.get_mut("characters").map(Value::take) {
        Some(Value::Array(characters)) if characters.is_empty() => Err("That account has no characters to import.".into()),
        Some(Value::Array(characters)) => Ok(characters),
        _ => Err("pathofexile.com sent a character list this app could not read".into()),
    }
}

#[tauri::command]
pub async fn ggg_character(app: AppHandle, state: State<'_, GggState>, game: State<'_, AppState>, realm: String, name: String) -> Result<String, String> {
    let realm = realm_path(game.game(), &realm)?;
    let reply = api_get(&app, &state, &format!("character{realm}"), &format!("/character{realm}/{}", query_value(&name))).await?;
    match reply.get("character") {
        Some(character) if character.is_object() => Ok(character.to_string()),
        _ => Err("pathofexile.com has no such character.".into()),
    }
}

fn trade_policy(url: &str) -> Result<&'static str, String> {
    TRADE_API.iter().copied().find(|prefix| url.starts_with(prefix)).ok_or_else(|| "not a pathofexile.com trade request".to_string())
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TradeReply {
    Reply { status: u16, body: String },
    Wait { seconds: u64 },
}

/// A rate limit comes back as `Wait`, for the caller to send the same request again after it.
#[tauri::command]
pub async fn ggg_trade_request(app: AppHandle, state: State<'_, GggState>, url: String, body: Option<String>) -> Result<TradeReply, String> {
    let policy = trade_policy(&url)?;
    match &state.store(&app).await.session {
        None => return Err(SIGNED_OUT.into()),
        Some(session) if !session.allows_trade() => return Err(NEEDS_TRADE.into()),
        Some(_) => {}
    }
    if let Some(seconds) = state.wait(policy) {
        return Ok(TradeReply::Wait { seconds });
    }
    let reply = send(&app, &state, policy, |client| match &body {
        Some(json) => client.post(&url).header("content-type", "application/json").body(json.clone()),
        None => client.get(&url),
    })
    .await?;
    match reply.status {
        403 => Err(NEEDS_TRADE.into()),
        429 => Ok(TradeReply::Wait { seconds: reply.wait.max(1) }),
        status => Ok(TradeReply::Reply { status, body: reply.body }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        pairs.iter().map(|(k, v)| (reqwest::header::HeaderName::from_static(k), reqwest::header::HeaderValue::from_static(v))).collect()
    }

    #[test]
    fn rate_limit_waits() {
        assert_eq!(wait_from(&headers(&[])), 0);
        let under = [("x-rate-limit-rules", "Account,Ip"), ("x-rate-limit-account", "5:10:60"), ("x-rate-limit-account-state", "2:10:0"), ("x-rate-limit-ip", "20:60:120"), ("x-rate-limit-ip-state", "3:60:0")];
        assert_eq!(wait_from(&headers(&under)), 0);
        let at_limit = [("x-rate-limit-rules", "account"), ("x-rate-limit-account", "5:10:60,30:300:600"), ("x-rate-limit-account-state", "5:10:0,7:300:0")];
        assert_eq!(wait_from(&headers(&at_limit)), 10);
        let restricted = [("x-rate-limit-rules", "account"), ("x-rate-limit-account", "5:10:60"), ("x-rate-limit-account-state", "6:10:60"), ("retry-after", "45")];
        assert_eq!(wait_from(&headers(&restricted)), 60);
        assert_eq!(wait_from(&headers(&[("retry-after", "7")])), 7);
    }

    #[test]
    fn redirect_parsing() {
        assert_eq!(callback("/callback?code=a%2Bb&state=s1", "s1"), Some(Ok("a+b".into())));
        assert_eq!(callback("/callback?code=abc&state=other", "s1"), None);
        assert_eq!(callback("/favicon.ico", "s1"), None);
        let denied = callback("/callback?error=access_denied&error_description=The+user+said+no&state=s1", "s1");
        assert_eq!(denied, Some(Err("pathofexile.com did not sign you in: The user said no".into())));
    }

    #[test]
    fn trade_requests_stay_on_the_trade_api() {
        assert!(trade_policy("https://www.pathofexile.com/api/trade2/search/poe2/Standard").is_ok());
        assert!(trade_policy("https://www.pathofexile.com/api/trade/fetch/a,b?query=q").is_ok());
        assert!(trade_policy("https://www.pathofexile.com/api/trade2/data/static").is_err());
        assert!(trade_policy("https://www.pathofexile.com/api/trade2/data/leagues").is_ok());
        assert!(trade_policy("https://www.pathofexile.com.example.com/api/trade2/search/x").is_err());
        assert!(trade_policy("https://api.pathofexile.com/character").is_err());
    }

    #[test]
    fn trade_needs_its_scope() {
        let session = |scope: &str| Session { account: String::new(), access: String::new(), refresh: None, expires_at: 0, ends_at: 0, scope: scope.into() };
        assert!(session("account:profile account:characters account:trade").allows_trade());
        assert!(!session("account:profile account:characters").allows_trade());
        assert!(!session("").allows_trade());
    }

    #[test]
    fn realms() {
        assert_eq!(realm_path(Game::Poe1, "pc"), Ok(""));
        assert_eq!(realm_path(Game::Poe1, "Sony"), Ok("/sony"));
        assert_eq!(realm_path(Game::Poe2, "pc"), Ok("/poe2"));
        assert!(realm_path(Game::Poe1, "switch").is_err());
    }

    #[tokio::test]
    async fn browser_redirect_reaches_the_waiter() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let waiter = tokio::spawn(wait_for_code(listener, "s1".into()));
        let idle = TcpStream::connect(addr).await.unwrap();
        let mut stray = TcpStream::connect(addr).await.unwrap();
        stray.write_all(b"GET /callback?code=wrong&state=nope HTTP/1.1\r\nHost: x\r\n\r\n").await.unwrap();
        let mut reply = String::new();
        stray.read_to_string(&mut reply).await.unwrap();
        assert!(reply.starts_with("HTTP/1.1 404"));
        let mut browser = TcpStream::connect(addr).await.unwrap();
        browser.write_all(b"GET /callback?code=c0de&state=s1 HTTP/1.1\r\nHost: x\r\n\r\n").await.unwrap();
        reply.clear();
        browser.read_to_string(&mut reply).await.unwrap();
        assert!(reply.starts_with("HTTP/1.1 200") && reply.contains("Signed in."));
        assert_eq!(waiter.await.unwrap(), Ok("c0de".into()));
        drop(idle);
    }
}
