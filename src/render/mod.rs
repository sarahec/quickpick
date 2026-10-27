//! Plan renderers: human-readable output and the git/jujutsu script dialects.

pub mod git;
pub mod jj;
pub mod message;
pub mod propose;

use git2::Oid;

use crate::plan::CommitInfo;

/// Quote a string for POSIX shell single-quoting: `'` becomes `'\''`.
pub fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// A here-document delimiter that does not appear as a line in `body`:
/// `QUICKPICK_EOF`, then `QUICKPICK_EOF_1`, `_2`, … until safe.
pub fn heredoc_delim(body: &str) -> String {
    let mut i = 0;
    loop {
        let delim = if i == 0 {
            "QUICKPICK_EOF".to_string()
        } else {
            format!("QUICKPICK_EOF_{i}")
        };
        if !body.lines().any(|l| l == delim) {
            return delim;
        }
        i += 1;
    }
}

/// The first seven hex characters of `oid`.
pub fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_string()
}

/// The last path component of `path`.
pub fn last_component(path: &str) -> &str {
    std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
}

/// The commit's author date as `YYYY-MM-DD` in the author's own timezone.
pub fn author_date(info: &CommitInfo) -> String {
    let secs = info.author_time + i64::from(info.author_offset_minutes) * 60;
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

/// Days since the Unix epoch to `(year, month, day)`, Howard Hinnant's
/// `civil_from_days`.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sh_quote_escapes_single_quotes() {
        assert_eq!(sh_quote("plain/path"), "'plain/path'");
        assert_eq!(sh_quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn heredoc_delim_avoids_body_lines() {
        assert_eq!(heredoc_delim("nothing"), "QUICKPICK_EOF");
        assert_eq!(heredoc_delim("QUICKPICK_EOF\nmore"), "QUICKPICK_EOF_1");
        assert_eq!(
            heredoc_delim("QUICKPICK_EOF\nQUICKPICK_EOF_1"),
            "QUICKPICK_EOF_2"
        );
    }

    #[test]
    fn last_component_takes_the_file_name() {
        assert_eq!(last_component("pkgs/by-name/op/openai"), "openai");
        assert_eq!(last_component("file.nix"), "file.nix");
    }

    #[test]
    fn author_date_uses_the_authors_offset() {
        let info = |time, off| CommitInfo {
            id: Oid::ZERO_SHA1,
            summary: String::new(),
            message: String::new(),
            author_time: time,
            author_offset_minutes: off,
        };
        // 2023-12-31T23:30:00Z at +13:00 is already 2024-01-01 locally.
        assert_eq!(author_date(&info(1_704_065_400, 13 * 60)), "2024-01-01");
        // 2024-01-01T01:00:00Z at -05:00 is still 2023-12-31 locally.
        assert_eq!(author_date(&info(1_704_070_800, -5 * 60)), "2023-12-31");
        assert_eq!(author_date(&info(0, 0)), "1970-01-01");
    }
}
