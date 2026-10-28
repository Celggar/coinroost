//! Tables drawn with ruling lines, found the way pdfplumber's `Page.extract_tables` finds them with its
//! default settings (the "lines" strategy, after Anssi Nurminen's thesis and Tabula). pdfplumber is MIT
//! licensed; this ports its algorithm step by step, because descriptions that span several lines go into
//! the transaction id and must come out identical.
//!
//! 1. The page's ruling edges are found and merged ([`super::edges`]).
//! 2. Every crossing of a vertical and a horizontal edge, within 3 points, is an intersection.
//! 3. Each intersection starts at most one cell: the smallest rectangle whose four corners are
//!    intersections joined by common edges.
//! 4. Cells that share a corner form a table; a table needs at least two cells.
//! 5. Cells are arranged into rows by their exact top and into columns by their exact left side (a merged
//!    cell leaves the columns it spans empty). The text of a cell is that of the glyphs whose centre lies
//!    inside it.

// Positions are compared exactly where the reference compares them exactly (see above).
#![allow(clippy::float_cmp)]

use std::collections::{HashMap, HashSet};

use super::edges::{Edge, Orientation, Shape, find_edges, py_max, py_min};
use super::layout::{Glyph, cell_text, py_cmp};

/// One row of a table: the text of each cell, or `None` where a merged cell covers the column.
pub type Row = Vec<Option<String>>;

const INTERSECTION_TOLERANCE: f64 = 3.0;

/// A cell as `(x0, top, x1, bottom)`.
type Cell = (f64, f64, f64, f64);

/// Every table of one page, top to bottom, as rows of cell text.
pub fn extract_tables(glyphs: &[Glyph], shapes: &[Shape]) -> Vec<Vec<Row>> {
    let intersections = Intersections::of(&find_edges(shapes));
    cells_to_tables(&intersections.cells())
        .iter()
        .map(|cells| extract(cells, glyphs))
        .collect()
}

/// A position as a map key; `-0.0` and `0.0` are the same key, as in Python.
fn key(point: (f64, f64)) -> (u64, u64) {
    ((point.0 + 0.0).to_bits(), (point.1 + 0.0).to_bits())
}

struct Intersection {
    point: (f64, f64),
    vertical: Vec<Edge>,
    horizontal: Vec<Edge>,
}

struct Intersections {
    by_point: HashMap<(u64, u64), usize>,
    /// Sorted by point.
    list: Vec<Intersection>,
}

impl Intersections {
    fn of(edges: &[Edge]) -> Intersections {
        let pick = |o: Orientation| -> Vec<Edge> {
            edges
                .iter()
                .filter(|e| e.orientation == Some(o))
                .copied()
                .collect()
        };
        let mut vertical = pick(Orientation::Vertical);
        let mut horizontal = pick(Orientation::Horizontal);
        vertical.sort_by(|a, b| py_cmp(a.x0, b.x0).then(py_cmp(a.top, b.top)));
        horizontal.sort_by(|a, b| py_cmp(a.top, b.top).then(py_cmp(a.x0, b.x0)));
        let tolerance = INTERSECTION_TOLERANCE;
        let mut by_point: HashMap<(u64, u64), usize> = HashMap::new();
        let mut list: Vec<Intersection> = Vec::new();
        for v in &vertical {
            for h in &horizontal {
                if v.top <= h.top + tolerance
                    && v.bottom >= h.top - tolerance
                    && v.x0 >= h.x0 - tolerance
                    && v.x0 <= h.x1 + tolerance
                {
                    let point = (v.x0, h.top);
                    let index = *by_point.entry(key(point)).or_insert_with(|| {
                        list.push(Intersection {
                            point,
                            vertical: Vec::new(),
                            horizontal: Vec::new(),
                        });
                        list.len() - 1
                    });
                    list[index].vertical.push(*v);
                    list[index].horizontal.push(*h);
                }
            }
        }
        list.sort_by(|a, b| py_cmp(a.point.0, b.point.0).then(py_cmp(a.point.1, b.point.1)));
        let by_point = list.iter().enumerate().map(|(i, x)| (key(x.point), i)).collect();
        Intersections { by_point, list }
    }

    fn at(&self, point: (f64, f64)) -> Option<&Intersection> {
        self.by_point.get(&key(point)).map(|&i| &self.list[i])
    }

    /// Whether one edge touches both intersections: a vertical one when they share `x`, a horizontal one
    /// when they share `y`.
    fn connected(a: &Intersection, b: &Intersection) -> bool {
        let share = |x: &[Edge], y: &[Edge]| x.iter().any(|e| y.iter().any(|f| e.same_box(f)));
        (a.point.0 == b.point.0 && share(&a.vertical, &b.vertical))
            || (a.point.1 == b.point.1 && share(&a.horizontal, &b.horizontal))
    }

    /// The smallest cell each intersection starts, in intersection order.
    fn cells(&self) -> Vec<Cell> {
        (0..self.list.len().saturating_sub(1))
            .filter_map(|i| self.smallest_cell(i))
            .collect()
    }

    fn smallest_cell(&self, i: usize) -> Option<Cell> {
        let corner = &self.list[i];
        let rest = &self.list[i + 1..];
        let (x, y) = corner.point;
        for below in rest.iter().filter(|p| p.point.0 == x) {
            if !Self::connected(corner, below) {
                continue;
            }
            for right in rest.iter().filter(|p| p.point.1 == y) {
                if !Self::connected(corner, right) {
                    continue;
                }
                let Some(opposite) = self.at((right.point.0, below.point.1)) else {
                    continue;
                };
                if Self::connected(opposite, right) && Self::connected(opposite, below) {
                    return Some((x, y, right.point.0, below.point.1));
                }
            }
        }
        None
    }
}

/// Groups cells that share a corner into tables, top to bottom, dropping single cells.
fn cells_to_tables(cells: &[Cell]) -> Vec<Vec<Cell>> {
    let corners = |c: &Cell| [(c.0, c.1), (c.0, c.3), (c.2, c.1), (c.2, c.3)];
    let mut taken = vec![false; cells.len()];
    let mut left = cells.len();
    let mut tables: Vec<Vec<Cell>> = Vec::new();
    let mut current: Vec<Cell> = Vec::new();
    let mut current_corners: HashSet<(u64, u64)> = HashSet::new();
    while left > 0 {
        let before = current.len();
        // One pass over the cells still free, in order; a cell joins when it touches the table so far.
        for (index, cell) in cells.iter().enumerate() {
            if taken[index] {
                continue;
            }
            if current.is_empty() || corners(cell).iter().any(|c| current_corners.contains(&key(*c))) {
                current_corners.extend(corners(cell).iter().map(|c| key(*c)));
                current.push(*cell);
                taken[index] = true;
                left -= 1;
            }
        }
        if current.len() == before {
            tables.push(std::mem::take(&mut current));
            current_corners.clear();
        }
    }
    if !current.is_empty() {
        tables.push(current);
    }
    let origin = |table: &Vec<Cell>| {
        table
            .iter()
            .map(|c| (c.1, c.0))
            .reduce(|a, b| if tuple_lt(b, a) { b } else { a })
            .expect("tables are never empty")
    };
    tables.sort_by(|a, b| {
        let (oa, ob) = (origin(a), origin(b));
        py_cmp(oa.0, ob.0).then(py_cmp(oa.1, ob.1))
    });
    tables.retain(|t| t.len() > 1);
    tables
}

fn tuple_lt(a: (f64, f64), b: (f64, f64)) -> bool {
    a.0 < b.0 || (a.0 == b.0 && a.1 < b.1)
}

/// Rows by exact top, columns by exact left side, then the text of every cell.
fn extract(cells: &[Cell], glyphs: &[Glyph]) -> Vec<Row> {
    let mut sorted = cells.to_vec();
    sorted.sort_by(|a, b| py_cmp(a.1, b.1).then(py_cmp(a.0, b.0)));
    let mut columns: Vec<f64> = cells.iter().map(|c| c.0).collect();
    columns.sort_by(|a, b| py_cmp(*a, *b));
    columns.dedup_by(|a, b| a == b);
    sorted
        .chunk_by(|a, b| a.1 == b.1)
        .map(|row| {
            let line: Vec<Option<Cell>> = columns
                .iter()
                .map(|x| row.iter().rev().find(|c| c.0 == *x).copied())
                .collect();
            row_text(&line, glyphs)
        })
        .collect()
}

fn row_text(line: &[Option<Cell>], glyphs: &[Glyph]) -> Row {
    let present = || line.iter().flatten();
    let bounds = (
        present().map(|c| c.0).reduce(py_min).unwrap_or(0.0),
        present().map(|c| c.1).reduce(py_min).unwrap_or(0.0),
        present().map(|c| c.2).reduce(py_max).unwrap_or(0.0),
        present().map(|c| c.3).reduce(py_max).unwrap_or(0.0),
    );
    let in_row: Vec<&Glyph> = glyphs.iter().filter(|g| centre_inside(g, bounds)).collect();
    line.iter()
        .map(|cell| {
            cell.map(|cell| {
                let inside: Vec<&Glyph> = in_row
                    .iter()
                    .copied()
                    .filter(|g| centre_inside(g, cell))
                    .collect();
                cell_text(&inside)
            })
        })
        .collect()
}

/// The glyph's centre lies in the box, left and top sides included.
fn centre_inside(glyph: &Glyph, (x0, top, x1, bottom): Cell) -> bool {
    let middle_y = f64::midpoint(glyph.top, glyph.bottom);
    let middle_x = f64::midpoint(glyph.x0, glyph.x1);
    middle_x >= x0 && middle_x < x1 && middle_y >= top && middle_y < bottom
}

#[cfg(test)]
mod tests {
    use super::{Glyph, extract_tables};
    use crate::pdf::edges::{Shape, ShapeKind};

    fn line(x0: f64, top: f64, x1: f64, bottom: f64) -> Shape {
        Shape {
            kind: ShapeKind::Line,
            x0,
            x1,
            top,
            bottom,
            width: x1 - x0,
            height: bottom - top,
            points: vec![(x0, top), (x1, bottom)],
        }
    }

    fn text(t: &str, x: f64, top: f64) -> Vec<Glyph> {
        t.chars()
            .enumerate()
            .map(|(i, c)| {
                let x0 = x + 5.0 * f64::from(u32::try_from(i).unwrap());
                Glyph {
                    text: c.to_string(),
                    x0,
                    x1: x0 + 5.0,
                    top,
                    bottom: top + 8.0,
                    upright: true,
                }
            })
            .collect()
    }

    #[test]
    fn reads_a_ruled_grid_with_a_merged_cell() {
        // Two columns, three rows; the last row has one cell across both columns. One rule is half a point
        // off, as real generators draw them, and snaps to the others; a sloped line is ignored.
        let shapes = vec![
            line(40.0, 100.0, 300.0, 100.0),
            line(40.0, 120.0, 300.0, 120.0),
            line(40.0, 141.0, 300.0, 141.0),
            line(40.0, 160.0, 300.0, 160.0),
            line(40.0, 100.0, 40.0, 160.0),
            line(300.5, 100.0, 300.5, 160.0),
            line(150.0, 100.0, 150.0, 141.0),
            line(10.0, 10.0, 20.0, 30.0),
        ];
        let mut glyphs = text("Fecha", 44.0, 106.0);
        glyphs.extend(text("Monto", 154.0, 106.0));
        glyphs.extend(text("01", 44.0, 126.0));
        glyphs.extend(text("TWO", 154.0, 124.0));
        glyphs.extend(text("LINES", 154.0, 132.0));
        glyphs.extend(text("TOTAL", 44.0, 146.0));
        let some = |s: &str| Some(s.to_owned());
        assert_eq!(
            extract_tables(&glyphs, &shapes),
            [vec![
                vec![some("Fecha"), some("Monto")],
                vec![some("01"), some("TWO\nLINES")],
                vec![some("TOTAL"), None],
            ]]
        );
    }
}
