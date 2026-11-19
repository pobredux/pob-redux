//! Build-sharing sites, mirroring `buildSites.websiteList` in PoB's
//! Modules/BuildSiteTools.lua for both games. Each download endpoint returns
//! the raw PoB build code (base64+deflate), which the bridge's
//! `load_build_code` decodes. Links are recognised by host and path whichever
//! game is active; uploads go to the active game's endpoints.

use crate::game::Game;

pub const SUPPORTED: &str = "Maxroll, Mobalytics, pobb.in, pob.codes, poe.ninja, poe2db.tw, poedb.tw, Pastebin.com, PastebinP.com, Rentry.co";

/// Map a share URL to (site label, raw-code download URL).
pub fn download_url(url: &str) -> Option<(&'static str, String)> {
    let rest = url.trim();
    let rest = rest.strip_prefix("https://").or_else(|| rest.strip_prefix("http://"))?;
    let rest = rest.strip_prefix("www.").unwrap_or(rest);
    let (host, path) = rest.split_once('/')?;
    let path = path.trim_end_matches('/');
    if path.is_empty() {
        return None;
    }
    let first_seg = path.split('/').next().unwrap_or_default();
    match host {
        "maxroll.gg" => {
            if let Some(id) = path.strip_prefix("poe2/pob/").filter(|s| !s.is_empty()) {
                return Some(("Maxroll", format!("https://maxroll.gg/poe2/api/pob/{id}")));
            }
            let id = path.strip_prefix("poe/pob/").filter(|s| !s.is_empty())?;
            Some(("Maxroll", format!("https://maxroll.gg/poe/api/pob/{id}")))
        }
        "pobb.in" => Some(("pobb.in", format!("https://pobb.in/pob/{path}"))),
        "pob.codes" => {
            let id = path.strip_prefix("b/").filter(|s| !s.is_empty())?;
            Some(("pob.codes", format!("https://api.pob.codes/{id}/raw")))
        }
        "poe.ninja" | "poe2.ninja" => {
            // poe.ninja/poe2/pob/<id>, poe2.ninja/pob/<id>, poe.ninja/poe1/pob/<id>, poe.ninja/pob/<id>
            let poe2 = host == "poe2.ninja" || path.starts_with("poe2/");
            let p = path.strip_prefix("poe2/").or_else(|| path.strip_prefix("poe1/")).unwrap_or(path);
            let id = p.strip_prefix("pob/").filter(|s| !s.is_empty())?;
            let game = if poe2 { "poe2" } else { "poe1" };
            Some(("poe.ninja", format!("https://poe.ninja/{game}/pob/raw/{id}")))
        }
        "poe2db.tw" => {
            let id = path.strip_prefix("pob/").filter(|s| !s.is_empty())?;
            Some(("poe2db.tw", format!("https://poe2db.tw/pob/{id}/raw")))
        }
        "poedb.tw" => {
            let id = path.strip_prefix("pob/").filter(|s| !s.is_empty())?;
            Some(("poedb.tw", format!("https://poedb.tw/pob/{id}/raw")))
        }
        "pastebin.com" => Some(("Pastebin.com", format!("https://pastebin.com/raw/{first_seg}"))),
        "pastebinp.com" => Some(("PastebinP.com", format!("https://pastebinp.com/raw/{first_seg}"))),
        "rentry.co" => Some(("Rentry.co", format!("https://rentry.co/paste/{first_seg}/raw"))),
        _ => None,
    }
}

/// Where a build code can be uploaded to make a share link, mirroring the
/// `postUrl` / `postFields` / `codeOut` columns of PoB's site table.
pub struct UploadTarget {
    pub label: &'static str,
    pub post_url: &'static str,
    /// Form prefix the code is appended to; empty when the body is the bare code.
    pub post_fields: &'static str,
    /// Prepended to the response to form the link.
    pub code_out: &'static str,
}

const UPLOAD_POE2: &[UploadTarget] = &[
    UploadTarget { label: "pobb.in", post_url: "https://pobb.in/pob/", post_fields: "", code_out: "https://pobb.in/" },
    UploadTarget { label: "Maxroll", post_url: "https://maxroll.gg/poe2/api/pob", post_fields: "pobCode=", code_out: "https://maxroll.gg/poe2/pob/" },
    UploadTarget { label: "poe.ninja", post_url: "https://poe.ninja/poe2/pob/api/upload", post_fields: "code=", code_out: "" },
    UploadTarget { label: "poe2db.tw", post_url: "https://poe2db.tw/pob/api/gen", post_fields: "", code_out: "" },
];

const UPLOAD_POE1: &[UploadTarget] = &[
    UploadTarget { label: "pobb.in", post_url: "https://pobb.in/pob/", post_fields: "", code_out: "https://pobb.in/" },
    UploadTarget { label: "Maxroll", post_url: "https://maxroll.gg/poe/api/pob", post_fields: "pobCode=", code_out: "https://maxroll.gg/poe/pob/" },
    UploadTarget { label: "poe.ninja", post_url: "https://poe.ninja/poe1/pob/api/upload", post_fields: "code=", code_out: "" },
    UploadTarget { label: "poedb.tw", post_url: "https://poedb.tw/pob/api/gen", post_fields: "", code_out: "" },
];

pub fn upload_targets(game: Game) -> &'static [UploadTarget] {
    match game {
        Game::Poe1 => UPLOAD_POE1,
        Game::Poe2 => UPLOAD_POE2,
    }
}

pub fn upload_target(game: Game, site: &str) -> Option<&'static UploadTarget> {
    let want = site.trim().to_ascii_lowercase();
    upload_targets(game).iter().find(|t| t.label.to_ascii_lowercase() == want)
}

#[cfg(test)]
mod tests {
    use super::{download_url, upload_target, Game};

    #[test]
    fn upload_targets_are_downloadable() {
        // A link made by an upload must be one the importer recognises.
        for game in [Game::Poe1, Game::Poe2] {
            assert_eq!(download_url(&format!("{}abc", upload_target(game, "pobb.in").unwrap().code_out)).unwrap().0, "pobb.in");
            assert_eq!(download_url(&format!("{}abc", upload_target(game, "maxroll").unwrap().code_out)).unwrap().0, "Maxroll");
            assert!(upload_target(game, "pastebin.com").is_none());
        }
        assert!(upload_target(Game::Poe1, "poe2db.tw").is_none());
        assert!(upload_target(Game::Poe2, "poedb.tw").is_none());
    }

    #[test]
    fn maps_urls() {
        assert_eq!(download_url("https://pobb.in/abc123").unwrap().1, "https://pobb.in/pob/abc123");
        assert_eq!(download_url("https://maxroll.gg/poe2/pob/xyz").unwrap().1, "https://maxroll.gg/poe2/api/pob/xyz");
        assert_eq!(download_url("https://maxroll.gg/poe/pob/xyz").unwrap().1, "https://maxroll.gg/poe/api/pob/xyz");
        assert_eq!(download_url("https://poe.ninja/poe2/pob/abc").unwrap().1, "https://poe.ninja/poe2/pob/raw/abc");
        assert_eq!(download_url("https://poe2.ninja/pob/abc").unwrap().1, "https://poe.ninja/poe2/pob/raw/abc");
        assert_eq!(download_url("https://poe.ninja/pob/abc").unwrap().1, "https://poe.ninja/poe1/pob/raw/abc");
        assert_eq!(download_url("https://poe.ninja/poe1/pob/abc").unwrap().1, "https://poe.ninja/poe1/pob/raw/abc");
        assert_eq!(
            download_url("https://poe.ninja/poe2/pob/overview/code?account=Some-1234&name=Char&overview=runes-of-aldur").unwrap().1,
            "https://poe.ninja/poe2/pob/raw/overview/code?account=Some-1234&name=Char&overview=runes-of-aldur"
        );
        assert_eq!(download_url("https://poedb.tw/pob/abc").unwrap().1, "https://poedb.tw/pob/abc/raw");
        assert_eq!(download_url("https://pob.codes/b/abc").unwrap().1, "https://api.pob.codes/abc/raw");
        assert!(download_url("https://pob.codes/").is_none());
        assert_eq!(download_url("https://pastebin.com/AbC123").unwrap().1, "https://pastebin.com/raw/AbC123");
        assert_eq!(download_url("https://rentry.co/mypaste").unwrap().1, "https://rentry.co/paste/mypaste/raw");
        assert!(download_url("https://example.com/whatever").is_none());
        assert!(download_url("https://maxroll.gg/d4/build/x").is_none());
    }
}
