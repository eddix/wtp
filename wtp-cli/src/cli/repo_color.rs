//! Deterministic, stable colors for repo names in `wtp ls --long`.
//!
//! Each repo is hashed to a fixed entry in a curated 24-bit palette, so the
//! same repo always shows in the same color across workspaces and across runs.
//! This makes the handful of frequently-used repos easy to spot at a glance.

use std::borrow::Cow;
use std::io::IsTerminal;

use wtp_core::RepoColorMode;

/// Curated 24-bit palette: distinct, legible hues chosen to read well on dark
/// terminals. We deliberately avoid pure red and pure green, which carry git
/// status semantics elsewhere in the output.
const PALETTE: &[(u8, u8, u8)] = &[
    (0x7a, 0xa2, 0xf7), // blue
    (0xbb, 0x9a, 0xf7), // purple
    (0xf7, 0x9a, 0xd3), // pink
    (0x9e, 0xce, 0x6a), // lime
    (0xe0, 0xaf, 0x68), // amber
    (0x2a, 0xc3, 0xde), // sky
    (0x73, 0xda, 0xca), // teal
    (0xff, 0x9e, 0x64), // orange
    (0xc0, 0xca, 0xf5), // periwinkle
    (0xff, 0xc7, 0x77), // gold
    (0xd2, 0xa6, 0xff), // lavender
    (0x86, 0xe1, 0xa9), // mint
    (0xf7, 0x76, 0x8e), // rose
    (0xb4, 0xd2, 0x73), // olive
];

/// FNV-1a (64-bit). Small, dependency-free, and stable across runs and
/// processes — unlike `DefaultHasher` it does not use a random seed — so a
/// given repo key always maps to the same palette entry.
fn fnv1a(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// The palette RGB a given key hashes to.
fn palette_rgb(key: &str) -> (u8, u8, u8) {
    PALETTE[(fnv1a(key) % PALETTE.len() as u64) as usize]
}

/// Whether color should actually be emitted for `mode`. Resolve this once per
/// command (it depends only on process-global state — the output mode, whether
/// stdout is a TTY, and `NO_COLOR`) and pass the result to [`paint_repo`].
pub fn should_color(mode: RepoColorMode) -> bool {
    match mode {
        RepoColorMode::Never => false,
        RepoColorMode::Always => true,
        RepoColorMode::Auto => {
            std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
        }
    }
}

/// Replace terminal control characters (C0/C1, including ESC) with the Unicode
/// replacement character.
///
/// Repo display strings ultimately come from `worktree.toml`, whose `host`/
/// `path` fields are user/config controlled. Without this, a crafted entry
/// could smuggle escape sequences (cursor moves, screen clears, OSC title or
/// hyperlink codes) onto the terminal whenever the workspace is listed.
fn sanitize(display: &str) -> Cow<'_, str> {
    if display.chars().any(char::is_control) {
        Cow::Owned(
            display
                .chars()
                .map(|c| if c.is_control() { '\u{FFFD}' } else { c })
                .collect(),
        )
    } else {
        Cow::Borrowed(display)
    }
}

/// Render a repo `display` string left-padded to `width` visible columns,
/// wrapped in its stable hashed color when `colorize` is true.
///
/// Control characters are stripped first (terminal-injection guard), and the
/// padding is applied to the plain text *before* the color escapes are added,
/// so the (zero-width) ANSI bytes never throw off column alignment.
pub fn paint_repo(display: &str, width: usize, colorize: bool) -> String {
    let safe = sanitize(display);
    let padded = format!("{safe:<width$}");
    if colorize {
        let (r, g, b) = palette_rgb(&safe);
        format!("\x1b[38;2;{r};{g};{b}m{padded}\x1b[0m")
    } else {
        padded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_deterministic() {
        assert_eq!(fnv1a("byted:oec/i18n_sdk"), fnv1a("byted:oec/i18n_sdk"));
        assert_ne!(fnv1a("byted:oec/i18n_sdk"), fnv1a("byted:oec/recas"));
    }

    #[test]
    fn same_repo_same_color() {
        assert_eq!(
            palette_rgb("byted:oec/i18n_sdk"),
            palette_rgb("byted:oec/i18n_sdk")
        );
    }

    #[test]
    fn color_index_in_range() {
        // Every key must map to a real palette entry.
        for key in ["a", "b", "byted:oec/recas", "x/y/z", ""] {
            assert!(PALETTE.contains(&palette_rgb(key)));
        }
    }

    #[test]
    fn not_colorized_is_plain_and_padded() {
        let out = paint_repo("repo", 10, false);
        assert_eq!(out, "repo      "); // padded to 10, no escapes
        assert!(!out.contains('\x1b'));
    }

    #[test]
    fn colorized_wraps_in_truecolor_escape() {
        let out = paint_repo("repo", 4, true);
        assert!(out.starts_with("\x1b[38;2;"));
        assert!(out.ends_with("\x1b[0m"));
        assert!(out.contains("repo"));
    }

    #[test]
    fn padding_uses_visible_width_not_escape_bytes() {
        // The visible content (between the SGR set and reset) must be exactly
        // `width` columns, regardless of the escape bytes around it.
        let (r, g, b) = palette_rgb("ab");
        let out = paint_repo("ab", 6, true);
        let inner = out
            .strip_prefix(&format!("\x1b[38;2;{r};{g};{b}m"))
            .and_then(|s| s.strip_suffix("\x1b[0m"))
            .unwrap();
        assert_eq!(inner, "ab    ");
    }

    #[test]
    fn should_color_respects_mode() {
        assert!(!should_color(RepoColorMode::Never));
        assert!(should_color(RepoColorMode::Always));
        // Auto depends on TTY / NO_COLOR; just ensure it evaluates without panic.
        let _ = should_color(RepoColorMode::Auto);
    }

    #[test]
    fn control_chars_are_stripped() {
        // An embedded ESC sequence in a repo name must never reach the terminal,
        // colorized or not.
        let plain = paint_repo("a\x1b[2Jb", 0, false);
        assert!(!plain.contains('\x1b'));
        assert!(plain.contains('\u{FFFD}'));

        let colored = paint_repo("a\x1b[2Jb", 0, true);
        // The only ESC is the leading SGR sequence we add ourselves.
        assert_eq!(colored.matches('\x1b').count(), 2); // SGR set + reset
        assert!(colored.contains('\u{FFFD}'));
    }
}
