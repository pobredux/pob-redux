use std::time::Duration;

use serde::{Deserialize, Serialize};

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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    url: String,
    #[serde(rename = "type")]
    kind: Option<String>,
    version: String,
    snapshot_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CharacterBuild {
    path_of_building_export: Option<String>,
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

pub async fn build_code(game: Game, account: &str, character: &str, league_url: &str) -> Result<String, String> {
    const NO_BUILD: &str = "poe.ninja has no build for that character.";
    let client = client()?;
    let q = crate::character::query_value;
    let url = format!("{HOST}{}/api/data/snapshot-versions?league={}", game.id(), q(league_url));
    let body = get(&client, &url).await?.ok_or("poe.ninja did not return its league list")?;
    let snapshots: Vec<Snapshot> = serde_json::from_str(&body).map_err(|_| "poe.ninja sent a league list this app could not read".to_string())?;
    // PoE1 leagues have an experience ladder and a delve depth ladder; both hold the same characters
    let snapshot = snapshots
        .iter()
        .filter(|s| s.url == league_url)
        .min_by_key(|s| !matches!(s.kind.as_deref(), None | Some("exp")))
        .ok_or("poe.ninja no longer has build data for that league.")?;
    let url = format!(
        "{HOST}{}/api/builds/{}/character?account={}&name={}&overview={}&timeMachine=",
        game.id(),
        q(&snapshot.version),
        q(&ninja_account(account)),
        q(character),
        q(&snapshot.snapshot_name),
    );
    let body = get(&client, &url).await?.ok_or(NO_BUILD)?;
    let build: CharacterBuild = serde_json::from_str(&body).map_err(|_| "poe.ninja sent a character this app could not read".to_string())?;
    build.path_of_building_export.filter(|c| !c.trim().is_empty()).ok_or_else(|| NO_BUILD.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_names() {
        assert_eq!(ninja_account(" ohitsjudd#7248 "), "ohitsjudd-7248");
        assert_eq!(ninja_account("ohitsjudd-7248"), "ohitsjudd-7248");
        assert_eq!(ninja_account("some-name#12"), "some-name-12");
        assert_eq!(hash_account("some-name-12"), "some-name#12");
    }

    #[test]
    fn parse_snapshot_versions() {
        let json = r#"[
            {"url":"covens","type":"exp","name":"Covens","version":"1336-20261119-16250","snapshotName":"covens"},
            {"url":"covens","type":"depthsolo","name":"Covens","version":"1342-20261119-14292","snapshotName":"covens"}
        ]"#;
        let snapshots: Vec<Snapshot> = serde_json::from_str(json).unwrap();
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].snapshot_name, "covens");
        assert_eq!(snapshots[0].kind.as_deref(), Some("exp"));
    }
}
