//! Shell path escaping for drag-and-drop file insertion
//!
//! When files are dragged onto a VTE terminal, their paths must be
//! properly escaped so the shell interprets them as literal filenames
//! rather than expanding special characters.

/// Escapes a file path for safe insertion into a POSIX shell.
///
/// Wraps the path in single quotes and escapes any embedded single quotes
/// using the `'\''` idiom (end quote, escaped quote, start quote).
///
/// # Examples
///
/// ```
/// use rustconn_core::shell_escape::escape_path;
///
/// assert_eq!(escape_path("/home/user/file.txt"), "'/home/user/file.txt'");
/// assert_eq!(escape_path("/tmp/my file"), "'/tmp/my file'");
/// assert_eq!(escape_path("/tmp/it's here"), "'/tmp/it'\\''s here'");
/// ```
#[must_use]
pub fn escape_path(path: &str) -> String {
    // Single-quote wrapping is the safest POSIX shell escaping method.
    // The only character that needs special handling inside single quotes
    // is the single quote itself.
    let mut escaped = String::with_capacity(path.len() + 2);
    escaped.push('\'');
    for ch in path.chars() {
        if ch == '\'' {
            // End current quote, add escaped single quote, restart quote
            escaped.push_str("'\\''");
        } else {
            escaped.push(ch);
        }
    }
    escaped.push('\'');
    escaped
}

/// Escapes multiple file paths and joins them with spaces.
///
/// Each path is individually escaped, then concatenated with a single
/// space separator — matching the behavior of GNOME Terminal.
///
/// # Examples
///
/// ```
/// use rustconn_core::shell_escape::escape_paths;
///
/// let paths = vec!["/tmp/a.txt", "/tmp/b c.txt"];
/// assert_eq!(escape_paths(&paths), "'/tmp/a.txt' '/tmp/b c.txt'");
/// ```
#[must_use]
pub fn escape_paths(paths: &[&str]) -> String {
    paths
        .iter()
        .map(|p| escape_path(p))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_path() {
        assert_eq!(escape_path("/home/user/file.txt"), "'/home/user/file.txt'");
    }

    #[test]
    fn test_path_with_spaces() {
        assert_eq!(
            escape_path("/home/user/my documents/file.txt"),
            "'/home/user/my documents/file.txt'"
        );
    }

    #[test]
    fn test_path_with_single_quote() {
        assert_eq!(escape_path("/tmp/it's here"), "'/tmp/it'\\''s here'");
    }

    #[test]
    fn test_path_with_special_chars() {
        assert_eq!(
            escape_path("/tmp/$HOME & stuff; rm -rf"),
            "'/tmp/$HOME & stuff; rm -rf'"
        );
    }

    #[test]
    fn test_path_with_newline() {
        assert_eq!(escape_path("/tmp/line\nbreak"), "'/tmp/line\nbreak'");
    }

    #[test]
    fn test_path_with_backtick() {
        assert_eq!(escape_path("/tmp/`whoami`"), "'/tmp/`whoami`'");
    }

    #[test]
    fn test_multiple_paths() {
        let paths = vec!["/tmp/a.txt", "/home/user/b c.txt", "/var/log/it's.log"];
        assert_eq!(
            escape_paths(&paths),
            "'/tmp/a.txt' '/home/user/b c.txt' '/var/log/it'\\''s.log'"
        );
    }

    #[test]
    fn test_empty_path() {
        assert_eq!(escape_path(""), "''");
    }

    #[test]
    fn test_path_with_unicode() {
        assert_eq!(escape_path("/tmp/файл.txt"), "'/tmp/файл.txt'");
    }
}
