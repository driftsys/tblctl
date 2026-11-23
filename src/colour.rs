//! Decides whether cell colour carries meaning.
//!
//! Colour is decorative when every data cell's colour depends only on its
//! column and the parity of its row (column colours and row bands).

use std::collections::HashMap;

/// Cell colour as (fill, font). Each is an optional ARGB hex string.
// Task 2 (the XLSX style reader) uses this; remove the allow then.
#[allow(dead_code)]
pub type Colour = (Option<String>, Option<String>);

/// Returns `true` when two data cells with the same column and the same row
/// parity have different colours. Each item is (row, col, colour).
// Task 2 (the XLSX style reader) uses this; remove the allow then.
#[allow(dead_code)]
pub fn varies(cells: &[(u32, u32, Colour)]) -> bool {
    let mut seen: HashMap<(u32, u32), &Colour> = HashMap::new();
    for (row, col, colour) in cells {
        match seen.get(&(*col, row % 2)) {
            Some(first) if *first != colour => return true,
            Some(_) => {}
            None => {
                seen.insert((*col, row % 2), colour);
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn red() -> Colour {
        (Some("FFFF0000".to_string()), None)
    }

    fn none() -> Colour {
        (None, None)
    }

    fn grid(f: impl Fn(u32, u32) -> Colour) -> Vec<(u32, u32, Colour)> {
        let mut cells = Vec::new();
        for row in 1..=6 {
            for col in 0..=2 {
                cells.push((row, col, f(row, col)));
            }
        }
        cells
    }

    #[test]
    fn uniform_colour_is_decorative() {
        assert!(!varies(&grid(|_, _| red())));
    }

    #[test]
    fn column_colour_is_decorative() {
        let cells = grid(|_, col| if col == 1 { red() } else { none() });
        assert!(!varies(&cells));
    }

    #[test]
    fn row_bands_are_decorative() {
        let cells = grid(|row, _| if row % 2 == 0 { red() } else { none() });
        assert!(!varies(&cells));
    }

    #[test]
    fn bands_plus_column_colour_is_decorative() {
        let cells = grid(|row, col| {
            let fill = (row % 2 == 0).then(|| "FFEEEEEE".to_string());
            let font = (col == 2).then(|| "FF0000FF".to_string());
            (fill, font)
        });
        assert!(!varies(&cells));
    }

    #[test]
    fn one_red_cell_is_meaningful() {
        let cells = grid(|row, col| if (row, col) == (3, 1) { red() } else { none() });
        assert!(varies(&cells));
    }

    #[test]
    fn coloured_total_row_is_meaningful() {
        let cells = grid(|row, _| if row == 6 { red() } else { none() });
        assert!(varies(&cells));
    }
}
