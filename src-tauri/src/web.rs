//! The assistant's web lookups. Every address and redirect must be on one of `SITES`.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use regex::Regex;
use reqwest::Url;
use serde_json::{json, Value};

use crate::game::Game;
use crate::tools::{JsonObject, ToolDef, ToolError};

pub(crate) const SITES: [&str; 6] = ["poe2wiki.net", "poe2db.tw", "pathofexile2.com", "poewiki.net", "poedb.tw", "pathofexile.com"];

const UA: &str = "Path of Building (PoB Redux)";
const MAX_BYTES: usize = 4 << 20;
const PART: usize = 4500;
const EXCERPT: usize = 700;
const MAX_MATCHES: usize = 6;

pub(crate) fn defs(game: Game) -> Vec<ToolDef> {
    let (wiki, db, sites) = match game {
        Game::Poe2 => ("poe2wiki.net", "poe2db.tw", "poe2db.tw, poe2wiki.net, pathofexile2.com and pathofexile.com"),
        Game::Poe1 => ("poewiki.net", "poedb.tw", "poedb.tw, poewiki.net and pathofexile.com"),
    };
    let notes = match game {
        Game::Poe2 => "https://www.pathofexile.com/forum/view-forum/2212",
        Game::Poe1 => "https://www.pathofexile.com/forum/view-forum/patch-notes",
    };
    let lookup = |name, description: String, schema| ToolDef {
        name,
        description,
        schema,
        output_schema: None,
        read_only: true,
        destructive: false,
        idempotent: true,
        open_world: true,
        slow: false,
    };
    let query = || {
        json!({
            "type": "object",
            "properties": { "query": { "type": "string", "description": "What to search for, in a few words" } },
            "required": ["query"],
            "additionalProperties": false
        })
    };
    vec![
        lookup(
            "search_poedb",
            format!(
                "The first place to look up a game fact the app's own tools do not cover. Searches {db}, the database built from the game's own files: items, uniques, mods, skill and support gems, passives, ascendancies, keywords and mechanics. Returns page names with their kind and address; read one with read_web_page. The user approves each lookup."
            ),
            query(),
        ),
        lookup(
            "search_wiki",
            format!(
                "Search {wiki}, the community wiki. Use it after search_poedb, when {db} has no page or the question is about patch history, drop sources or how a mechanic plays. Returns page titles, addresses and snippets; read one with read_web_page. The user approves each lookup."
            ),
            query(),
        ),
        lookup(
            "read_web_page",
            format!(
                "Read a web page as text. Only {sites} can be read; any other address is refused. Patch notes are listed at {notes}. Long pages come in parts: pass `find` to jump to the lines you need, or `offset` from the previous part's next_offset. The user approves each lookup."
            ),
            json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Full https address of the page" },
                    "find": { "type": "string", "description": "Text to look for; returns the passages around each match instead of the start" },
                    "offset": { "type": "integer", "description": "Character offset to read from (default 0)" }
                },
                "required": ["url"],
                "additionalProperties": false
            }),
        ),
    ]
}

pub(crate) async fn run(game: Game, name: &str, args: &JsonObject) -> Option<Result<Value, ToolError>> {
    let text = |key: &str| args.get(key).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());
    let out = match name {
        "search_poedb" => {
            let Some(query) = text("query") else { return Some(Err(ToolError::Invalid("query is required".into()))) };
            search_db(game, query).await
        }
        "search_wiki" => {
            let Some(query) = text("query") else { return Some(Err(ToolError::Invalid("query is required".into()))) };
            search_wiki(game, query).await
        }
        "read_web_page" => {
            let Some(url) = text("url") else { return Some(Err(ToolError::Invalid("url is required".into()))) };
            let offset = args.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
            read_page(url, text("find"), offset).await
        }
        _ => return None,
    };
    Some(out.map_err(ToolError::Failed))
}

fn site_of(host: &str) -> Option<&'static str> {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    SITES.into_iter().find(|s| host == *s || host.strip_suffix(s).is_some_and(|rest| rest.ends_with('.')))
}

fn allowed(url: &Url) -> bool {
    url.scheme() == "https" && url.port().is_none() && url.username().is_empty() && url.host_str().is_some_and(|h| site_of(h).is_some())
}

fn checked(raw: &str) -> Result<Url, String> {
    let mut url = Url::parse(raw).map_err(|_| format!("{raw} is not a web address."))?;
    if url.scheme() == "http" {
        let _ = url.set_scheme("https");
    }
    url.set_fragment(None);
    if !allowed(&url) {
        return Err(format!("{raw} is not on a site the assistant may read. Only {} are allowed.", SITES.join(", ")));
    }
    Ok(url)
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(UA)
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 {
                attempt.error("too many redirects")
            } else if allowed(attempt.url()) {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .build()
        .map_err(|e| e.to_string())
}

async fn fetch(url: &Url) -> Result<(Url, String), String> {
    let host = url.host_str().unwrap_or_default().to_string();
    let mut resp = client()?.get(url.clone()).send().await.map_err(|e| format!("{host}: {e}"))?;
    let status = resp.status();
    if status.is_redirection() {
        let to = resp.headers().get(reqwest::header::LOCATION).and_then(|v| v.to_str().ok()).unwrap_or("another site");
        return Err(format!("{url} redirects to {to}, which is not a site the assistant may read."));
    }
    match status.as_u16() {
        200..=299 => {}
        404 | 410 => return Err(format!("No page at {url}.")),
        429 => return Err(format!("{host} is limiting requests. Wait a minute, then try again.")),
        code => return Err(format!("{host} returned HTTP {code}.")),
    }
    let kind = resp.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("").to_ascii_lowercase();
    if !(kind.is_empty() || kind.starts_with("text/") || kind.contains("json") || kind.contains("xml")) {
        return Err(format!("{url} is not a text page ({kind})."));
    }
    let final_url = resp.url().clone();
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| format!("{host}: {e}"))? {
        body.extend_from_slice(&chunk);
        if body.len() >= MAX_BYTES {
            break;
        }
    }
    Ok((final_url, String::from_utf8_lossy(&body).into_owned()))
}

async fn fetch_json(url: &Url) -> Result<Value, String> {
    let (_, body) = fetch(url).await?;
    serde_json::from_str(&body).map_err(|e| format!("{}: {e}", url.host_str().unwrap_or_default()))
}

struct Entry {
    label: String,
    lower: String,
    kind: String,
    url: String,
}

fn db_host(game: Game) -> &'static str {
    match game {
        Game::Poe2 => "poe2db.tw",
        Game::Poe1 => "poedb.tw",
    }
}

/// The site's own search box list: every page name with its kind.
async fn db_index(game: Game) -> Result<Arc<Vec<Entry>>, String> {
    static INDEX: OnceLock<Mutex<[Option<(Instant, Arc<Vec<Entry>>)>; 2]>> = OnceLock::new();
    let slot = usize::from(game == Game::Poe2);
    let index = INDEX.get_or_init(|| Mutex::new([None, None]));
    if let Some((at, entries)) = &index.lock().unwrap()[slot] {
        if at.elapsed() < Duration::from_secs(6 * 3600) {
            return Ok(entries.clone());
        }
    }
    let host = db_host(game);
    let file = match game {
        Game::Poe2 => "autocompletecb_us.json",
        Game::Poe1 => "autocomplete_us.json",
    };
    let v = fetch_json(&checked(&format!("https://{host}/json/{file}"))?).await?;
    let entries: Vec<Entry> = v
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let label = e["label"].as_str()?.trim().to_string();
            let value = e["value"].as_str()?.trim();
            let url = match value {
                v if v.starts_with("https://") => v.to_string(),
                v if v.starts_with('/') => format!("https://{host}{v}"),
                v => format!("https://{host}/us/{v}"),
            };
            Some(Entry { lower: label.to_lowercase(), label, kind: e["desc"].as_str().unwrap_or_default().to_string(), url })
        })
        .collect();
    if entries.is_empty() {
        return Err(format!("{host} returned an empty page list."));
    }
    let entries = Arc::new(entries);
    index.lock().unwrap()[slot] = Some((Instant::now(), entries.clone()));
    Ok(entries)
}

/// Pages whose name holds every word, or failing that the most words; the flag says which.
fn rank<'a>(entries: &'a [Entry], query: &str, limit: usize) -> (Vec<&'a Entry>, bool) {
    let q = query.trim().to_lowercase();
    let words: Vec<&str> = q.split_whitespace().collect();
    let mut hits: Vec<(u8, usize, &Entry)> = entries
        .iter()
        .filter_map(|e| {
            let score = if e.lower == q {
                4
            } else if e.lower.starts_with(&q) {
                3
            } else if e.lower.contains(&q) {
                2
            } else if words.iter().all(|w| e.lower.contains(w)) {
                1
            } else {
                return None;
            };
            Some((score, e.label.len(), e))
        })
        .collect();
    if !hits.is_empty() {
        hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        return (hits.into_iter().take(limit).map(|h| h.2).collect(), false);
    }
    let mut partial: Vec<(usize, bool, usize, &Entry)> = entries
        .iter()
        .filter_map(|e| {
            let n = words.iter().filter(|w| w.len() > 2 && e.lower.contains(**w)).count();
            (n > 0).then_some((n, e.kind != "Keywords", e.label.len(), e))
        })
        .collect();
    partial.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    (partial.into_iter().take(limit).map(|h| h.3).collect(), true)
}

async fn search_db(game: Game, query: &str) -> Result<Value, String> {
    let entries = db_index(game).await?;
    let (hits, partial) = rank(&entries, query, 12);
    let results: Vec<Value> = hits.into_iter().map(|e| json!({ "title": e.label, "kind": e.kind, "url": e.url })).collect();
    let mut out = json!({ "site": db_host(game), "results": results });
    if results.is_empty() {
        out["note"] = json!("Nothing matched. Try other words, or search_wiki.");
    } else if partial {
        out["note"] = json!("No page name has every word. These have some of them; a mechanic is often explained on its keyword page.");
    }
    Ok(out)
}

fn wiki_host(game: Game) -> &'static str {
    match game {
        Game::Poe2 => "www.poe2wiki.net",
        Game::Poe1 => "www.poewiki.net",
    }
}

fn wiki_url(host: &str, title: &str) -> String {
    format!("https://{host}/wiki/{}", crate::character::query_value(&title.replace(' ', "_")))
}

async fn search_wiki(game: Game, query: &str) -> Result<Value, String> {
    let host = wiki_host(game);
    let url = checked(&format!(
        "https://{host}/api.php?action=query&list=search&srsearch={}&srlimit=8&srprop=snippet&format=json&formatversion=2",
        crate::character::query_value(query)
    ))?;
    let v = fetch_json(&url).await?;
    if let Some(e) = v["error"]["info"].as_str() {
        return Err(format!("{host}: {e}"));
    }
    let results: Vec<Value> = v["query"]["search"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let title = r["title"].as_str()?;
            Some(json!({ "title": title, "url": wiki_url(host, title), "snippet": html_text(r["snippet"].as_str().unwrap_or_default()) }))
        })
        .collect();
    let mut out = json!({ "wiki": host, "results": results });
    if results.is_empty() {
        out["note"] = json!("Nothing matched. Try fewer or different words.");
    }
    Ok(out)
}

struct Page {
    key: String,
    url: String,
    title: String,
    text: String,
    at: Instant,
}

fn cache() -> &'static Mutex<VecDeque<Arc<Page>>> {
    static CACHE: OnceLock<Mutex<VecDeque<Arc<Page>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// A wiki article's own content, without the site's menus and sidebars.
async fn wiki_article(url: &Url) -> Option<Result<(String, String), String>> {
    let host = url.host_str()?;
    if !matches!(site_of(host), Some("poewiki.net" | "poe2wiki.net")) {
        return None;
    }
    let title = url.path().strip_prefix("/wiki/").filter(|t| !t.is_empty())?.replace('&', "%26").replace('+', "%2B");
    let api = match checked(&format!(
        "https://{host}/api.php?action=parse&page={title}&prop=text&format=json&formatversion=2&redirects=1&disableeditsection=1&disabletoc=1"
    )) {
        Ok(api) => api,
        Err(e) => return Some(Err(e)),
    };
    Some(fetch_json(&api).await.and_then(|v| {
        if let Some(e) = v["error"]["info"].as_str() {
            return Err(format!("{url}: {e}"));
        }
        let title = v["parse"]["title"].as_str().unwrap_or_default().to_string();
        Ok((title, html_text(v["parse"]["text"].as_str().unwrap_or_default())))
    }))
}

async fn load(url: Url) -> Result<Arc<Page>, String> {
    let key = url.to_string();
    {
        let mut pages = cache().lock().unwrap();
        pages.retain(|p| p.at.elapsed() < Duration::from_secs(600));
        if let Some(p) = pages.iter().find(|p| p.key == key) {
            return Ok(p.clone());
        }
    }
    let (url, title, text) = match wiki_article(&url).await {
        Some(article) => {
            let (title, text) = article?;
            (key.clone(), title, text)
        }
        None => {
            let (final_url, html) = fetch(&url).await?;
            (final_url.to_string(), page_title(&html), html_text(&html))
        }
    };
    let page = Arc::new(Page { key, url, title, text, at: Instant::now() });
    let mut pages = cache().lock().unwrap();
    pages.push_front(page.clone());
    pages.truncate(8);
    Ok(page)
}

async fn read_page(raw: &str, find: Option<&str>, offset: usize) -> Result<Value, String> {
    let page = load(checked(raw)?).await?;
    let text = page.text.as_str();
    let length = text.chars().count();
    let mut out = json!({ "url": page.url, "title": page.title, "length": length });
    match find {
        Some(needle) => {
            let re = Regex::new(&format!("(?i){}", regex::escape(needle))).map_err(|e| e.to_string())?;
            let mut matches: Vec<Value> = Vec::new();
            let mut covered = 0;
            for m in re.find_iter(text) {
                if m.start() < covered {
                    continue;
                }
                let start = floor_char(text, m.start().saturating_sub(EXCERPT / 3));
                let end = floor_char(text, (start + EXCERPT).min(text.len()));
                covered = end;
                matches.push(json!({ "offset": text[..start].chars().count(), "text": &text[start..end] }));
                if matches.len() == MAX_MATCHES {
                    break;
                }
            }
            if matches.is_empty() {
                out["note"] = json!(format!("No match for \"{needle}\". Try another word, or read from offset 0."));
            }
            out["matches"] = json!(matches);
        }
        None => {
            let offset = offset.min(length);
            let part: String = text.chars().skip(offset).take(PART).collect();
            out["offset"] = json!(offset);
            if offset + PART < length {
                out["next_offset"] = json!(offset + PART);
            }
            out["text"] = json!(part);
            if length == 0 {
                out["note"] = json!("This page shows no text without a browser. Try the wiki or another page.");
            }
        }
    }
    Ok(out)
}

fn floor_char(s: &str, mut i: usize) -> usize {
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn page_title(html: &str) -> String {
    static TITLE: OnceLock<Regex> = OnceLock::new();
    let re = TITLE.get_or_init(|| Regex::new(r"(?is)<title[^>]*>(.*?)</title>").unwrap());
    re.captures(html).map(|c| decode_entities(c[1].trim())).unwrap_or_default()
}

/// The wikis repeat a linked term's definition in a hidden hover box after the link.
fn drop_tooltips(html: &str) -> String {
    static RES: OnceLock<[Regex; 2]> = OnceLock::new();
    let [open, tag] = RES.get_or_init(|| {
        [
            Regex::new(r#"(?i)<(span|div)\b[^>]*class="[^"]*hoverbox(?:&#95;|_)_display[^"]*"[^>]*>"#).unwrap(),
            Regex::new(r"(?i)<(/?)(span|div)\b[^>]*>").unwrap(),
        ]
    });
    let mut out = String::with_capacity(html.len());
    let mut pos = 0;
    while let Some(m) = open.captures_at(html, pos) {
        let whole = m.get(0).unwrap();
        let name = m[1].to_ascii_lowercase();
        out.push_str(&html[pos..whole.start()]);
        pos = html.len();
        let mut depth = 1;
        for t in tag.captures_iter(&html[whole.end()..]) {
            if !t[2].eq_ignore_ascii_case(&name) {
                continue;
            }
            depth += if t[1].is_empty() { 1 } else { -1 };
            if depth == 0 {
                pos = whole.end() + t.get(0).unwrap().end();
                break;
            }
        }
    }
    out.push_str(&html[pos.min(html.len())..]);
    out
}

pub(crate) fn html_text(html: &str) -> String {
    static RES: OnceLock<[Regex; 7]> = OnceLock::new();
    let [chrome, comment, item, cell, block, tag, blank] = RES.get_or_init(|| {
        [
            Regex::new(
                r"(?is)<script\b.*?</script>|<style\b.*?</style>|<noscript\b.*?</noscript>|<svg\b.*?</svg>|<head\b.*?</head>|<nav\b.*?</nav>|<footer\b.*?</footer>|<form\b.*?</form>|<select\b.*?</select>|<template\b.*?</template>",
            )
            .unwrap(),
            Regex::new(r"(?s)<!--.*?-->").unwrap(),
            Regex::new(r"(?i)<li\b[^>]*>").unwrap(),
            Regex::new(r"(?i)</t[dh]>").unwrap(),
            Regex::new(r"(?i)<br\s*/?>|</(p|div|tr|h[1-6]|table|ul|ol|dl|dd|dt|section|article|blockquote|pre|caption)>").unwrap(),
            Regex::new(r"(?s)<[^>]*>").unwrap(),
            Regex::new(r"\n{3,}").unwrap(),
        ]
    });
    let s = chrome.replace_all(html, " ");
    let s = comment.replace_all(&s, " ");
    let s = drop_tooltips(&s);
    let s = item.replace_all(&s, "\n- ");
    let s = cell.replace_all(&s, " | ");
    let s = block.replace_all(&s, "\n");
    let s = tag.replace_all(&s, "");
    let s = decode_entities(&s);
    let lines: Vec<String> = s
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .map(|l| l.trim_matches(|c: char| c == '|' || c.is_whitespace()).to_string())
        .collect();
    blank.replace_all(lines.join("\n").trim(), "\n\n").into_owned()
}

fn decode_entities(s: &str) -> String {
    static ENTITY: OnceLock<Regex> = OnceLock::new();
    let re = ENTITY.get_or_init(|| Regex::new(r"&(#[xX][0-9a-fA-F]{1,6}|#[0-9]{1,7}|[a-zA-Z]{2,8});").unwrap());
    re.replace_all(s, |c: &regex::Captures| {
        let body = &c[1];
        let code = if let Some(hex) = body.strip_prefix("#x").or_else(|| body.strip_prefix("#X")) {
            u32::from_str_radix(hex, 16).ok()
        } else if let Some(dec) = body.strip_prefix('#') {
            dec.parse().ok()
        } else {
            None
        };
        if let Some(ch) = code.and_then(char::from_u32) {
            return ch.to_string();
        }
        match body {
            "amp" => "&",
            "lt" => "<",
            "gt" => ">",
            "quot" => "\"",
            "apos" => "'",
            "nbsp" => " ",
            "ndash" => "-",
            "mdash" => "-",
            "hellip" => "...",
            "lsquo" | "rsquo" => "'",
            "ldquo" | "rdquo" => "\"",
            "times" => "x",
            "minus" => "-",
            _ => return c[0].to_string(),
        }
        .to_string()
    })
    .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_listed_sites_pass() {
        assert!(checked("https://www.poe2wiki.net/wiki/Armour_Break").is_ok());
        assert!(checked("http://poe2db.tw/us/Armour_Break").is_ok());
        assert!(checked("https://www.pathofexile.com/forum/view-forum/2212").is_ok());
        assert!(checked("https://evilpoedb.tw/").is_err());
        assert!(checked("https://poedb.tw.evil.com/").is_err());
        assert!(checked("https://user@poedb.tw/").is_err());
        assert!(checked("https://poedb.tw:8443/").is_err());
        assert!(checked("ftp://poedb.tw/").is_err());
        assert!(checked("https://example.com/?u=https://poedb.tw").is_err());
    }

    #[test]
    fn html_becomes_text() {
        let html = "<html><head><title>T</title></head><body><nav>menu</nav><script>x()</script><h2>Armour&nbsp;Break</h2><p>Breaks <b>armour</b>.</p><ul><li>One</li><li>Two &amp; three</li></ul><table><tr><td>a</td><td>b</td></tr></table></body></html>";
        assert_eq!(html_text(html), "Armour Break\nBreaks armour.\n\n- One\n- Two & three\na | b");
    }

    #[test]
    fn exact_names_rank_first() {
        let entry = |label: &str, kind: &str| Entry { label: label.into(), lower: label.to_lowercase(), kind: kind.into(), url: String::new() };
        let entries = [
            entry("Armour Break III", "Support Gems"),
            entry("Sundered Armour", "Keywords"),
            entry("Armour Break", "Keywords"),
            entry("Break Armour on Hit", "Passive"),
        ];
        let names = |query| rank(&entries, query, 5).0.iter().map(|e| e.label.as_str()).collect::<Vec<_>>();
        assert_eq!(names("armour break"), ["Armour Break", "Armour Break III", "Break Armour on Hit"]);
        assert_eq!(names("fully broken armour"), ["Armour Break", "Sundered Armour", "Armour Break III", "Break Armour on Hit"]);
        assert!(rank(&entries, "zzqqxx", 5).0.is_empty());
    }

    #[test]
    fn hover_boxes_are_dropped() {
        let html = r#"Break <span class="hoverbox"><span class="hoverbox&#95;_activator"><a>Armour</a></span><span class="hoverbox&#95;_display"><span>Armour</span> reduces damage.</span></span> to zero."#;
        assert_eq!(html_text(html), "Break Armour to zero.");
    }
}
