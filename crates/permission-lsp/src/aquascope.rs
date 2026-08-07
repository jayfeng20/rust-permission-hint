//! Parsing of `cargo aquascope permissions` output and cursor lookup.
//!
//! Aquascope prints a JSON array of `Result<AnalysisOutput, AquascopeError>`
//! (externally tagged: `{"Ok": ...}` / `{"Err": ...}`). We model only the
//! fields we need; serde ignores the rest.

use serde::Deserialize;
use std::path::Path;
use std::process::Command;

/// The R/W/O permissions a place holds, in Aquascope's naming (`drop` == Own).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub(crate) struct Permissions {
    pub read: bool,
    pub write: bool,
    pub drop: bool,
}

/// Zero-based line/column of a permission boundary (start of a place expression).
#[derive(Debug, Clone, Copy, Deserialize)]
struct CharPos {
    line: usize,
    column: usize,
}

#[derive(Debug, Clone, Deserialize)]
struct PermissionsBoundary {
    location: CharPos,
    actual: Permissions,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AnalysisOutput {
    boundaries: Vec<PermissionsBoundary>,
}

/// Run `cargo aquascope permissions` in `crate_dir` and parse the result.
///
/// The project must be set up for Aquascope (its pinned nightly toolchain);
/// otherwise the command fails and the error is surfaced to the caller.
pub(crate) fn run(crate_dir: &Path) -> Result<Vec<AnalysisOutput>, String> {
    let output = Command::new("cargo")
        .args(["aquascope", "permissions"])
        .current_dir(crate_dir)
        .output()
        .map_err(|e| format!("could not run `cargo aquascope` (is it installed?): {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.lines().rev().find(|l| !l.trim().is_empty());
        return Err(format!(
            "`cargo aquascope` failed: {}",
            detail.unwrap_or("unknown error")
        ));
    }
    // The JSON is the last non-empty stdout line (cargo/miri noise goes to stderr).
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json = stdout
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("[]");
    parse(json).map_err(|e| format!("could not parse `cargo aquascope` output: {e}"))
}

/// Parse the CLI output, discarding bodies that failed to analyze (`Err`).
pub(crate) fn parse(json: &str) -> serde_json::Result<Vec<AnalysisOutput>> {
    // `Result<T, IgnoredAny>` consumes the `Err` payload without modeling it.
    let bodies: Vec<Result<AnalysisOutput, serde::de::IgnoredAny>> = serde_json::from_str(json)?;
    Ok(bodies.into_iter().flatten().collect())
}

/// The `actual` permissions of the boundary under the cursor, if any.
///
/// A boundary sits at the start of a place expression; hovering anywhere within
/// that token should resolve to it. We pick, among boundaries on the cursor's
/// line, the closest one starting at or before the cursor column.
pub(crate) fn permissions_at(
    bodies: &[AnalysisOutput],
    line: usize,
    column: usize,
) -> Option<Permissions> {
    bodies
        .iter()
        .flat_map(|b| &b.boundaries)
        .filter(|b| b.location.line == line && b.location.column <= column)
        .max_by_key(|b| b.location.column)
        .map(|b| b.actual)
}

impl Permissions {
    /// Render as a hover tooltip, e.g. `**Permissions:** R  W  ~~O~~`.
    pub(crate) fn to_hover_markdown(self) -> String {
        let mark = |held: bool, letter: char| {
            if held {
                format!("`{letter}`")
            } else {
                format!("~~{letter}~~")
            }
        };
        format!(
            "**Permissions:** {} {} {}",
            mark(self.read, 'R'),
            mark(self.write, 'W'),
            mark(self.drop, 'O'),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Shape mirrors real `cargo aquascope permissions` output: an array of
    // `{"Ok": {body_range, boundaries, steps, ...}}`, boundaries carrying
    // `location: {line, column}` and `actual: {read, write, drop}`. Extra
    // fields are present to prove they're ignored.
    const SAMPLE: &str = r#"
    [
      {"Ok": {
        "body_range": {"start": {"line": 0, "column": 0}, "end": {"line": 3, "column": 1}, "filename": 0},
        "steps": [],
        "boundaries": [
          {"location": {"line": 1, "column": 12},
           "expected": {"read": true, "write": false, "drop": false},
           "actual":   {"read": true, "write": false, "drop": true},
           "data": {}},
          {"location": {"line": 2, "column": 4},
           "expected": {"read": true, "write": true, "drop": false},
           "actual":   {"read": true, "write": true, "drop": false},
           "data": {}}
        ]
      }},
      {"Err": {"type": "BuildError", "range": null}}
    ]"#;

    #[test]
    fn parses_ok_bodies_and_skips_errors() {
        let bodies = parse(SAMPLE).unwrap();
        assert_eq!(bodies.len(), 1);
        assert_eq!(bodies[0].boundaries.len(), 2);
    }

    #[test]
    fn finds_boundary_when_cursor_is_within_the_token() {
        let bodies = parse(SAMPLE).unwrap();
        // Cursor a few chars into the token starting at (1, 12).
        let perms = permissions_at(&bodies, 1, 14).unwrap();
        assert_eq!(
            perms,
            Permissions {
                read: true,
                write: false,
                drop: true
            }
        );
    }

    #[test]
    fn picks_the_nearest_boundary_at_or_before_the_cursor() {
        let bodies = parse(SAMPLE).unwrap();
        // On line 2 there is one boundary at column 4; cursor at 6 resolves to it.
        let perms = permissions_at(&bodies, 2, 6).unwrap();
        assert_eq!(
            perms,
            Permissions {
                read: true,
                write: true,
                drop: false
            }
        );
    }

    #[test]
    fn no_boundary_before_the_cursor_returns_none() {
        let bodies = parse(SAMPLE).unwrap();
        // Column 0 on line 1 is before the only boundary (column 12).
        assert!(permissions_at(&bodies, 1, 0).is_none());
        // A line with no boundaries.
        assert!(permissions_at(&bodies, 9, 0).is_none());
    }

    #[test]
    fn empty_output_parses_to_nothing() {
        let bodies = parse("[]").unwrap();
        assert!(bodies.is_empty());
        assert!(permissions_at(&bodies, 0, 0).is_none());
    }
}
