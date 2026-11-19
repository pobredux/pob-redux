use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::webview::{NewWindowResponse, PageLoadEvent};
use tauri::{AppHandle, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::game::Game;

const HOST: &str = "https://poe.ninja/";
const UA: &str = concat!("PoB Redux/", env!("CARGO_PKG_VERSION"), " (+https://pobredux.com)");

const NO_PROFILE: &str = "poe.ninja has no profile for that account. Check the name, including its #1234 part.";
const LIMITED: &str = "poe.ninja is limiting requests. Wait a minute, then try again.";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProfileCharacter {
    account_name: Option<String>,
    name: String,
    level: Option<u32>,
    updated: Option<String>,
    #[serde(default)]
    is_current: bool,
    league: Option<String>,
    league_url: Option<String>,
    class_name: Option<String>,
    eligibility: Option<Eligibility>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Eligibility {
    #[serde(rename = "type")]
    kind: String,
    nominal_min_level: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Character {
    pub name: String,
    pub class_name: Option<String>,
    pub level: u32,
    pub league: String,
    pub league_url: String,
    pub updated: Option<String>,
    pub is_current: bool,
    pub status: String,
    pub min_level: Option<u32>,
}

#[derive(Serialize)]
pub struct CharacterList {
    pub account: String,
    pub characters: Vec<Character>,
}

fn ninja_account(name: &str) -> String {
    let name: String = name.chars().filter(|c| !c.is_whitespace()).collect();
    match name.rfind(['#', '-']) {
        Some(i) => format!("{}-{}", &name[..i], &name[i + 1..]),
        None => name,
    }
}

fn hash_account(name: &str) -> String {
    match name.rfind('-') {
        Some(i) => format!("{}#{}", &name[..i], &name[i + 1..]),
        None => name.to_string(),
    }
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder().user_agent(UA).build().map_err(|e| e.to_string())
}

async fn get(client: &reqwest::Client, url: &str) -> Result<Option<String>, String> {
    let resp = client.get(url).timeout(Duration::from_secs(20)).send().await.map_err(|e| format!("poe.ninja: {e}"))?;
    let status = resp.status().as_u16();
    match status {
        200..=299 => resp.text().await.map(Some).map_err(|e| format!("poe.ninja: {e}")),
        404 => Ok(None),
        429 => Err(LIMITED.into()),
        _ => Err(format!("poe.ninja returned HTTP {status}")),
    }
}

/// poe.ninja caches the character list per version and matches the account name case-sensitively.
async fn profile_version(client: &reqwest::Client, game: Game, account: &str) -> Result<Option<u64>, String> {
    let url = format!("{HOST}{}/api/events/characters/{account}", game.id());
    let mut resp = client
        .get(&url)
        .header("accept", "text/event-stream")
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("poe.ninja: {e}"))?;
    match resp.status().as_u16() {
        200..=299 => {}
        404 => return Ok(None),
        429 => return Err(LIMITED.into()),
        status => return Err(format!("poe.ninja returned HTTP {status}")),
    }
    // the stream stays open; only its first event is needed
    let mut buf = String::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| format!("poe.ninja: {e}"))? {
        buf.push_str(&String::from_utf8_lossy(&chunk).replace("\r\n", "\n"));
        while let Some(end) = buf.find("\n\n") {
            let event: String = buf.drain(..end + 2).collect();
            let name = event.lines().find_map(|l| l.strip_prefix("event:")).map(str::trim);
            let data: String = event.lines().filter_map(|l| l.strip_prefix("data:")).map(str::trim).collect();
            match name {
                Some("message") | None if !data.is_empty() => {
                    let version = serde_json::from_str::<serde_json::Value>(&data).ok().and_then(|v| v.get("version")?.as_u64());
                    if version.is_some() {
                        return Ok(version);
                    }
                }
                Some("message") | None => {}
                Some(_) => return Ok(None),
            }
        }
        if buf.len() > 16_000 {
            break;
        }
    }
    Ok(None)
}

pub async fn list(game: Game, account: &str) -> Result<CharacterList, String> {
    let mut account = ninja_account(account);
    if account.is_empty() {
        return Err("Enter an account name.".into());
    }
    let client = client()?;
    let mut version = profile_version(&client, game, &account).await?;
    if version.is_none() {
        if let Some(real) = crate::character::account_casing(&hash_account(&account)).await.filter(|r| *r != account) {
            version = profile_version(&client, game, &real).await?;
            account = real;
        }
    }
    let version = version.ok_or(NO_PROFILE)?;
    let body = get(&client, &format!("{HOST}{}/api/profile/characters/{account}/{version}", game.id())).await?.ok_or(NO_PROFILE)?;
    let found: Vec<ProfileCharacter> = serde_json::from_str(&body).map_err(|_| "poe.ninja sent a character list this app could not read".to_string())?;
    if found.is_empty() {
        return Err("That poe.ninja profile has no characters for this game.".into());
    }
    let account = found.iter().find_map(|c| c.account_name.clone()).unwrap_or_else(|| hash_account(&account));
    let characters = found
        .into_iter()
        .map(|c| Character {
            status: c.eligibility.as_ref().map_or_else(|| "unlisted".into(), |e| e.kind.clone()),
            min_level: c.eligibility.as_ref().and_then(|e| e.nominal_min_level),
            name: c.name,
            class_name: c.class_name,
            level: c.level.unwrap_or(0),
            league: c.league.unwrap_or_default(),
            league_url: c.league_url.unwrap_or_default(),
            updated: c.updated,
            is_current: c.is_current,
        })
        .collect();
    Ok(CharacterList { account, characters })
}

fn character_url(game: Game, account: &str, character: &str, league: &str) -> reqwest::Url {
    let mut url = reqwest::Url::parse(HOST).expect("valid poe.ninja base URL");
    url.path_segments_mut()
        .expect("poe.ninja URL supports path segments")
        .extend([game.id(), "builds", league, "character", &ninja_account(account), character]);
    url
}

async fn read_export(window: &WebviewWindow) -> Result<String, String> {
    // poe.ninja renders the public export field after its page JavaScript loads.
    const READ_CODE: &str = r#"(() => {
        const value = document.querySelector('input[readonly][aria-label="Import code for Path of Building"]')?.value?.trim();
        return value && /^[A-Za-z0-9_+/-]{100,250000}={0,2}$/.test(value) ? value : "";
    })()"#;
    loop {
        let (send, mut receive) = tokio::sync::mpsc::channel(1);
        window.eval_with_callback(READ_CODE, move |json| {
            let _ = send.try_send(serde_json::from_str::<String>(&json).unwrap_or_default());
        }).map_err(|e| e.to_string())?;
        if let Some(code) = receive.recv().await.filter(|code| !code.is_empty()) {
            return Ok(code);
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

pub async fn build_code(app: AppHandle, game: Game, account: &str, character: &str, league: &str) -> Result<String, String> {
    if league.is_empty() {
        return Err("poe.ninja did not provide the character's league.".into());
    }
    static NEXT_WINDOW: AtomicU64 = AtomicU64::new(0);
    let label = format!("ninja-import-{}", NEXT_WINDOW.fetch_add(1, Ordering::Relaxed));
    let url = character_url(game, account, character, league);
    let (loaded, mut load_event) = tokio::sync::mpsc::channel(1);
    let window = WebviewWindowBuilder::new(&app, label, WebviewUrl::External(url.clone()))
        .visible(false)
        .focused(false)
        .incognito(true)
        .on_navigation(move |target| target == &url)
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .on_download(|_, _| false)
        .on_page_load(move |_, event| {
            if event.event() == PageLoadEvent::Finished {
                let _ = loaded.try_send(());
            }
        })
        .build()
        .map_err(|e| e.to_string())?;
    let result = tokio::time::timeout(Duration::from_secs(9), async {
        load_event.recv().await.ok_or("poe.ninja closed before the page loaded.")?;
        read_export(&window).await
    }).await;
    let _ = window.destroy();
    result.map_err(|_| "poe.ninja did not display an export code in time.".to_string())?
}

#[cfg(test)]
mod tests {
    use super::{character_url, hash_account, ninja_account, Game};

    #[test]
    fn account_names() {
        assert_eq!(ninja_account(" ohitsjudd#7248 "), "ohitsjudd-7248");
        assert_eq!(ninja_account("ohitsjudd-7248"), "ohitsjudd-7248");
        assert_eq!(ninja_account("some-name#12"), "some-name-12");
        assert_eq!(hash_account("some-name-12"), "some-name#12");
    }

    #[test]
    fn character_pages_encode_each_segment() {
        assert_eq!(
            character_url(Game::Poe2, "Sample#0675", "ExampleCharacter", "forbiddenrites").as_str(),
            "https://poe.ninja/poe2/builds/forbiddenrites/character/Sample-0675/ExampleCharacter"
        );
        assert_eq!(
            character_url(Game::Poe1, "Some-name#1234", "Name/é", "hc covens").as_str(),
            "https://poe.ninja/poe1/builds/hc%20covens/character/Some-name-1234/Name%2F%C3%A9"
        );
    }
}
