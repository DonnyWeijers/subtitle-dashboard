//! Turns subtrans' free-text titles into show, season and episode.
//!
//! subtrans logs a title like `"Show - S01E02 - Episode name · file name.en.srt"` for an episode and
//! `"Film title · file name.en.srt"` for a film. Test runs start with `"TEST "` (costs.jsonl) or live under
//! `/test/` (state.json).

use std::sync::LazyLock;

use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShowKind {
    Series,
    Movie,
    Test,
}

impl ShowKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Series => "series",
            Self::Movie => "movie",
            Self::Test => "test",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Title {
    pub kind: ShowKind,
    /// Series name, film title, or "Tests".
    pub show: String,
    pub season: Option<i64>,
    pub episode: Option<i64>,
    pub episode_title: Option<String>,
    /// The subtitle file name after the " · ", if present.
    pub file: Option<String>,
}

pub const TEST_SHOW: &str = "Tests";

static EPISODE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?P<show>.+?) - S(?P<season>\d{1,3})E(?P<episode>\d{1,4})(?:-?E\d{1,4})*(?: - (?P<name>.*))?$")
        .expect("valid regex")
});

pub fn parse(title: &str) -> Title {
    let (display, file) = match title.split_once(" · ") {
        Some((display, file)) => (display.trim(), Some(file.trim().to_string()).filter(|f| !f.is_empty())),
        None => (title.trim(), None),
    };

    if display == "TEST" || display.starts_with("TEST ") {
        return Title {
            kind: ShowKind::Test,
            show: TEST_SHOW.to_string(),
            season: None,
            episode: None,
            episode_title: Some(display.to_string()),
            file,
        };
    }

    if let Some(c) = EPISODE.captures(display) {
        return Title {
            kind: ShowKind::Series,
            show: c["show"].trim().to_string(),
            season: c["season"].parse().ok(),
            episode: c["episode"].parse().ok(),
            episode_title: c.name("name").map(|m| m.as_str().trim().to_string()).filter(|s| !s.is_empty()),
            file,
        };
    }

    Title {
        kind: ShowKind::Movie,
        show: if display.is_empty() { "(unknown)".to_string() } else { display.to_string() },
        season: None,
        episode: None,
        episode_title: None,
        file,
    }
}

/// state.json is keyed by the subtitle's path inside Bazarr's container. A `/test/` path is a test run,
/// whatever its title says.
pub fn parse_with_path(path: &str, title: &str) -> Title {
    let mut parsed = parse(title);
    if path.starts_with("/test/") && parsed.kind != ShowKind::Test {
        parsed.episode_title = Some(parsed.show.clone());
        parsed.kind = ShowKind::Test;
        parsed.show = TEST_SHOW.to_string();
    }
    parsed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn episode_with_file() {
        let t = parse(
            "Harbour Watch: Night Shift - S28E03 - Anonymity · Harbour Watch - S28E03 - Anonymity WEBDL-1080p.en.srt",
        );
        assert_eq!(t.kind, ShowKind::Series);
        assert_eq!(t.show, "Harbour Watch: Night Shift");
        assert_eq!((t.season, t.episode), (Some(28), Some(3)));
        assert_eq!(t.episode_title.as_deref(), Some("Anonymity"));
        assert_eq!(t.file.as_deref(), Some("Harbour Watch - S28E03 - Anonymity WEBDL-1080p.en.srt"));
    }

    #[test]
    fn show_name_with_dashes() {
        let t = parse("Spy - Who - S01E02 - The Bridge - Part 2");
        assert_eq!(t.show, "Spy - Who");
        assert_eq!(t.episode_title.as_deref(), Some("The Bridge - Part 2"));
    }

    #[test]
    fn double_episode_and_no_name() {
        let t = parse("The Night Bakery - S09E09-E10 - The Final");
        assert_eq!((t.season, t.episode), (Some(9), Some(9)));
        assert_eq!(t.episode_title.as_deref(), Some("The Final"));

        let t = parse("The Night Bakery - S09E11");
        assert_eq!((t.kind, t.episode, t.episode_title), (ShowKind::Series, Some(11), None));
    }

    #[test]
    fn film() {
        let t = parse("Paper Lanterns: A Story · Paper Lanterns - A Story (2016) WEBDL-1080p.en.hi.srt");
        assert_eq!(t.kind, ShowKind::Movie);
        assert_eq!(t.show, "Paper Lanterns: A Story");
        assert_eq!(t.season, None);
    }

    #[test]
    fn test_runs() {
        let t = parse("TEST luna effort S08E86");
        assert_eq!((t.kind, t.show.as_str()), (ShowKind::Test, TEST_SHOW));

        let t = parse_with_path("/test/sample.en.srt", "Sample - S01E01 - Pilot");
        assert_eq!((t.kind, t.show.as_str()), (ShowKind::Test, TEST_SHOW));
        assert_eq!(t.episode_title.as_deref(), Some("Sample"));
    }

    #[test]
    fn empty_title() {
        assert_eq!(parse("  ").show, "(unknown)");
    }
}
