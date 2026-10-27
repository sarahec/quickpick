//! Plan renderers: human-readable output and the git/jujutsu script dialects.

pub mod message;

use git2::Oid;

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
}
