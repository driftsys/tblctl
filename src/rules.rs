//! Rules T0 to T3. The first rule that matches wins.

use crate::table::Grid;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// T1 size limits.
pub struct Limits {
    pub max_rows: usize,
    pub max_cols: usize,
    pub max_cell: usize,
}

#[derive(Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Rule {
    T0,
    T1,
    T2,
    T3,
}

#[derive(Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    Skip,
    Markdown,
    Csv,
    Link,
}

impl Rule {
    pub fn target(self) -> Target {
        match self {
            Rule::T0 => Target::Skip,
            Rule::T1 => Target::Markdown,
            Rule::T2 => Target::Csv,
            Rule::T3 => Target::Link,
        }
    }
}

pub struct Verdict {
    pub rule: Rule,
    /// Why the table did not get an earlier rule, or why it was skipped.
    pub reasons: Vec<String>,
}

/// Reasons why `grid` is over the T1 size limits; empty if it is within them.
pub fn size_reasons(grid: &Grid, limits: &Limits) -> Vec<String> {
    let mut reasons = Vec::new();
    let rows = grid.data_rows();
    let columns = grid.columns();
    let longest = grid.facts.markdown_longest_cell.unwrap_or_else(|| {
        grid.rows
            .iter()
            .flatten()
            .map(|c| c.chars().count())
            .max()
            .unwrap_or(0)
    });
    if rows > limits.max_rows {
        reasons.push(format!("rows {rows} > {}", limits.max_rows));
    }
    if columns > limits.max_cols {
        reasons.push(format!("columns {columns} > {}", limits.max_cols));
    }
    if longest > limits.max_cell {
        reasons.push(format!("cell {longest} > {}", limits.max_cell));
    }
    reasons
}

/// True if one header cell has the word `date` and a different header cell
/// has the word `author`, `description` or `modification` (any case).
fn is_revision_history(grid: &Grid) -> bool {
    let Some(header) = grid.rows.first() else {
        return false;
    };
    let has = |cell: &str, keys: &[&str]| {
        cell.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .any(|w| keys.contains(&w))
    };
    header.iter().enumerate().any(|(i, a)| {
        has(a, &["date"])
            && header
                .iter()
                .enumerate()
                .any(|(j, b)| j != i && has(b, &["author", "description", "modification"]))
    })
}

fn content_hash(grid: &Grid) -> [u8; 32] {
    let mut h = Sha256::new();
    for row in &grid.rows {
        for cell in row {
            h.update(cell.as_bytes());
            h.update([0x1f]);
        }
        h.update([0x1e]);
    }
    h.finalize().into()
}

/// Classifies every grid. `grids` is in input order; a duplicate is any
/// table whose content equals an earlier table's content.
pub fn classify(grids: &[Grid], limits: &Limits) -> Vec<Verdict> {
    let mut seen: HashMap<[u8; 32], String> = HashMap::new();
    grids
        .iter()
        .map(|g| {
            if g.rows.iter().flatten().all(|c| c.trim().is_empty()) {
                return Verdict {
                    rule: Rule::T0,
                    reasons: vec!["empty".into()],
                };
            }
            let hash = content_hash(g);
            if let Some(first) = seen.get(&hash) {
                return Verdict {
                    rule: Rule::T0,
                    reasons: vec![format!("duplicate of {first}")],
                };
            }
            seen.insert(hash, format!("{}#{}", g.file, g.name));
            if is_revision_history(g) {
                return Verdict {
                    rule: Rule::T0,
                    reasons: vec!["revision history".into()],
                };
            }
            let mut structure = Vec::new();
            if g.facts.visible_sheets > 1 {
                structure.push(format!("{} visible sheets", g.facts.visible_sheets));
            }
            if g.facts.data_merges {
                structure.push("merged data cells".into());
            }
            if g.facts.images {
                structure.push("images".into());
            }
            let size = size_reasons(g, limits);
            if structure.is_empty() && size.is_empty() {
                return Verdict {
                    rule: Rule::T1,
                    reasons: Vec::new(),
                };
            }
            let mut reasons = structure.clone();
            reasons.extend(size);
            if structure.is_empty() && !g.facts.formulas {
                return Verdict {
                    rule: Rule::T2,
                    reasons,
                };
            }
            if g.facts.formulas {
                reasons.push("formulas".into());
            }
            Verdict {
                rule: Rule::T3,
                reasons,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::Facts;

    const LIMITS: Limits = Limits {
        max_rows: 30,
        max_cols: 8,
        max_cell: 120,
    };

    fn grid(rows: &[&[&str]]) -> Grid {
        Grid {
            file: "f".into(),
            name: "s".into(),
            rows: rows
                .iter()
                .map(|r| r.iter().map(|c| c.to_string()).collect())
                .collect(),
            facts: Facts {
                visible_sheets: 1,
                ..Facts::default()
            },
        }
    }

    fn big() -> Grid {
        let mut g = grid(&[&["a", "b"]]);
        for i in 0..31 {
            g.rows.push(vec![i.to_string(), "x".into()]);
        }
        g
    }

    #[test]
    fn empty_and_duplicate_are_t0() {
        let v = classify(
            &[
                grid(&[&["", ""]]),
                grid(&[&["a"], &["1"]]),
                grid(&[&["a"], &["1"]]),
            ],
            &LIMITS,
        );
        assert_eq!(v[0].rule, Rule::T0);
        assert_eq!(v[0].reasons, ["empty"]);
        assert_eq!(v[1].rule, Rule::T1);
        assert_eq!(v[2].rule, Rule::T0);
        assert_eq!(v[2].reasons, ["duplicate of f#s"]);
    }

    #[test]
    fn revision_history_is_t0() {
        let v = classify(
            &[grid(&[
                &["Version", "Date", "Author"],
                &["1", "2020", "me"],
            ])],
            &LIMITS,
        );
        assert_eq!(v[0].rule, Rule::T0);
    }

    #[test]
    fn revision_history_needs_whole_words_in_two_columns() {
        for (header, want) in [
            (&["Date", "Description"][..], Rule::T0),
            (&["Release date", "Modification"][..], Rule::T0),
            (&["DATE", "AUTHOR"][..], Rule::T0),
            (&["Author", "Date"][..], Rule::T0),
            (&["Date:", "Author(s)"][..], Rule::T0),
            (&["Update", "Description"][..], Rule::T1),
            (&["Date", "Value"][..], Rule::T1),
            (&["Author", "Name"][..], Rule::T1),
            (&["Date of description"][..], Rule::T1),
            (&["Date", "Change"][..], Rule::T1),
            (&["Validated", "Authority"][..], Rule::T1),
        ] {
            let mut g = grid(&[header]);
            g.rows.push(vec!["1".into(); header.len()]);
            assert_eq!(classify(&[g], &LIMITS)[0].rule, want, "{header:?}");
        }
    }

    #[test]
    fn duplicate_names_the_earlier_table() {
        let mut a = grid(&[&["a"], &["1"]]);
        a.name = "first".into();
        let mut b = grid(&[&["a"], &["1"]]);
        b.file = "g".into();
        b.name = "second".into();
        let v = classify(&[a, b], &LIMITS);
        assert_eq!(v[1].reasons, ["duplicate of f#first"]);
    }

    #[test]
    fn limits_are_inclusive() {
        let mut at = grid(&[&["a"; 8]]);
        for _ in 0..30 {
            at.rows.push(vec!["x".repeat(120); 8]);
        }
        assert_eq!(classify(&[at], &LIMITS)[0].rule, Rule::T1);

        let mut rows = grid(&[&["a"]]);
        for i in 0..31 {
            rows.rows.push(vec![i.to_string()]);
        }
        let cols = grid(&[&["a"; 9], &["1"; 9]]);
        let long = "x".repeat(121);
        let cell = grid(&[&["a"], &[long.as_str()]]);
        let v = classify(&[rows, cols, cell], &LIMITS);
        assert_eq!(v[0].reasons, ["rows 31 > 30"]);
        assert_eq!(v[1].reasons, ["columns 9 > 8"]);
        assert_eq!(v[2].reasons, ["cell 121 > 120"]);
    }

    #[test]
    fn header_cells_count_toward_cell_length() {
        let long = "x".repeat(121);
        let g = grid(&[&[long.as_str()], &["1"]]);
        assert_eq!(classify(&[g], &LIMITS)[0].reasons, ["cell 121 > 120"]);
    }

    #[test]
    fn large_plain_table_is_t2() {
        let v = classify(&[big()], &LIMITS);
        assert_eq!(v[0].rule, Rule::T2);
        assert_eq!(v[0].reasons, ["rows 31 > 30"]);
    }

    #[test]
    fn large_table_with_formulas_is_t3() {
        let mut g = big();
        g.facts.formulas = true;
        assert_eq!(classify(&[g], &LIMITS)[0].rule, Rule::T3);
    }

    #[test]
    fn small_table_with_formulas_is_t1() {
        let mut g = grid(&[&["a"], &["1"]]);
        g.facts.formulas = true;
        assert_eq!(classify(&[g], &LIMITS)[0].rule, Rule::T1);
    }

    #[test]
    fn merged_data_cells_images_or_several_sheets_are_t3() {
        for set in [
            |f: &mut Facts| f.data_merges = true,
            |f: &mut Facts| f.images = true,
            |f: &mut Facts| f.visible_sheets = 2,
        ] {
            let mut g = grid(&[&["a"], &["1"]]);
            set(&mut g.facts);
            assert_eq!(classify(&[g], &LIMITS)[0].rule, Rule::T3);
        }
    }
}
