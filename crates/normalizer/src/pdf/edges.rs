//! Ruling edges of a page from its painted paths, merged the way pdfplumber merges them before it looks
//! for tables (default settings of its "lines" strategy).
//!
//! Every painted line, rectangle and other path becomes edges: a line one edge, a rectangle its four
//! sides, any other path one edge per segment between its points. Horizontal and vertical edges of at least
//! 1 point are kept. Edges within 3 points of each other across their direction are moved to their average
//! position; edges on the same line that overlap or lie within 3 points are joined. Edges shorter than 3
//! points are then dropped.
//!
//! Positions are compared exactly where pdfplumber compares them exactly, and averages use the compensated
//! sum of Python 3.12 and later, so snapped positions come out as the same numbers.

// Positions are compared exactly where the reference compares them exactly (see above).
#![allow(clippy::float_cmp)]

use super::layout::{cluster_ids, py_cmp};

const SNAP_TOLERANCE: f64 = 3.0;
const JOIN_TOLERANCE: f64 = 3.0;
const EDGE_MIN_LENGTH_PREFILTER: f64 = 1.0;
const EDGE_MIN_LENGTH: f64 = 3.0;

/// What a painted path was recognized as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    /// A single straight segment.
    Line,
    /// An axis-aligned closed rectangle.
    Rect,
    /// Anything else; only the end points of its segments are kept.
    Curve,
}

/// A painted path in page space, y pointing down. `width` and `height` are those of the box before it was
/// flipped, as pdfminer reports them.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub kind: ShapeKind,
    pub x0: f64,
    pub x1: f64,
    pub top: f64,
    pub bottom: f64,
    pub width: f64,
    pub height: f64,
    /// End points of the path's segments; edges of a curve run between consecutive points.
    pub points: Vec<(f64, f64)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Orientation {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Edge {
    pub x0: f64,
    pub x1: f64,
    pub top: f64,
    pub bottom: f64,
    pub width: f64,
    pub height: f64,
    pub orientation: Option<Orientation>,
}

impl Edge {
    fn length(&self) -> f64 {
        if self.orientation == Some(Orientation::Vertical) {
            self.height
        } else {
            self.width
        }
    }

    pub(super) fn same_box(&self, other: &Edge) -> bool {
        self.x0 == other.x0 && self.top == other.top && self.x1 == other.x1 && self.bottom == other.bottom
    }
}

/// Edges in pdfplumber's order (lines, then rectangle sides, then curve segments), merged and filtered.
pub(super) fn find_edges(shapes: &[Shape]) -> Vec<Edge> {
    let of_kind = |kind: ShapeKind| shapes.iter().filter(move |s| s.kind == kind);
    let page_edges: Vec<Edge> = of_kind(ShapeKind::Line)
        .map(line_edge)
        .chain(of_kind(ShapeKind::Rect).flat_map(rect_edges))
        .chain(of_kind(ShapeKind::Curve).flat_map(curve_edges))
        .collect();
    let mut edges = filter_edges(
        &page_edges,
        Some(Orientation::Vertical),
        EDGE_MIN_LENGTH_PREFILTER,
    );
    edges.extend(filter_edges(
        &page_edges,
        Some(Orientation::Horizontal),
        EDGE_MIN_LENGTH_PREFILTER,
    ));
    filter_edges(&merge_edges(edges), None, EDGE_MIN_LENGTH)
}

fn line_edge(line: &Shape) -> Edge {
    Edge {
        x0: line.x0,
        x1: line.x1,
        top: line.top,
        bottom: line.bottom,
        width: line.width,
        height: line.height,
        // A sloped line counts as vertical, as in pdfplumber.
        orientation: Some(if line.top == line.bottom {
            Orientation::Horizontal
        } else {
            Orientation::Vertical
        }),
    }
}

fn rect_edges(r: &Shape) -> [Edge; 4] {
    let side = |x0, x1, top, bottom, width, height, orientation| Edge {
        x0,
        x1,
        top,
        bottom,
        width,
        height,
        orientation: Some(orientation),
    };
    let (h, v) = (Orientation::Horizontal, Orientation::Vertical);
    [
        side(r.x0, r.x1, r.top, r.top, r.width, 0.0, h),
        side(r.x0, r.x1, r.top + r.height, r.bottom, r.width, 0.0, h),
        side(r.x0, r.x0, r.top, r.bottom, 0.0, r.height, v),
        side(r.x1, r.x1, r.top, r.bottom, 0.0, r.height, v),
    ]
}

fn curve_edges(curve: &Shape) -> Vec<Edge> {
    curve
        .points
        .windows(2)
        .map(|pair| {
            let ((ax, ay), (bx, by)) = (pair[0], pair[1]);
            Edge {
                x0: py_min(ax, bx),
                x1: py_max(ax, bx),
                top: py_min(ay, by),
                bottom: py_max(ay, by),
                width: (ax - bx).abs(),
                height: (ay - by).abs(),
                orientation: if ax == bx {
                    Some(Orientation::Vertical)
                } else if ay == by {
                    Some(Orientation::Horizontal)
                } else {
                    None
                },
            }
        })
        .collect()
}

/// Python's `min(a, b)`: `a` unless `b` is smaller.
pub(super) fn py_min(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// Python's `max(a, b)`: `a` unless `b` is larger.
pub(super) fn py_max(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

fn filter_edges(edges: &[Edge], orientation: Option<Orientation>, min_length: f64) -> Vec<Edge> {
    edges
        .iter()
        .filter(|e| orientation.is_none_or(|o| e.orientation == Some(o)) && e.length() >= min_length)
        .copied()
        .collect()
}

/// Snaps vertical edges by `x0` and horizontal ones by `top`, then joins the edges of each line.
fn merge_edges(edges: Vec<Edge>) -> Vec<Edge> {
    let (vertical, horizontal): (Vec<Edge>, Vec<Edge>) = edges
        .into_iter()
        .partition(|e| e.orientation == Some(Orientation::Vertical));
    let mut snapped = snap(vertical, Orientation::Vertical);
    snapped.extend(snap(horizontal, Orientation::Horizontal));
    // Grouped by ("h", top) or ("v", x0): horizontal lines first, as those tuples sort.
    let key = |e: &Edge| match e.orientation {
        Some(Orientation::Vertical) => (1, e.x0),
        _ => (0, e.top),
    };
    snapped.sort_by(|a, b| {
        let (ka, kb) = (key(a), key(b));
        ka.0.cmp(&kb.0).then(py_cmp(ka.1, kb.1))
    });
    let mut merged = Vec::with_capacity(snapped.len());
    for group in snapped.chunk_by(|a, b| key(a) == key(b)) {
        merged.extend(join(group));
    }
    merged
}

/// Moves each edge to the average position of its cluster: `x + (average - x)`, as pdfplumber computes it.
fn snap(edges: Vec<Edge>, orientation: Orientation) -> Vec<Edge> {
    let position = |e: &Edge| match orientation {
        Orientation::Vertical => e.x0,
        Orientation::Horizontal => e.top,
    };
    let ids = cluster_ids(&edges.iter().map(position).collect::<Vec<_>>(), SNAP_TOLERANCE);
    let mut tagged: Vec<(usize, Edge)> = ids.into_iter().zip(edges).collect();
    tagged.sort_by_key(|(id, _)| *id);
    let mut out = Vec::with_capacity(tagged.len());
    for cluster in tagged.chunk_by(|a, b| a.0 == b.0) {
        #[allow(clippy::cast_precision_loss)]
        let average = python_sum(cluster.iter().map(|(_, e)| position(e))) / cluster.len() as f64;
        for (_, edge) in cluster {
            let mut edge = *edge;
            let shift = average - position(&edge);
            match orientation {
                Orientation::Vertical => {
                    edge.x0 += shift;
                    edge.x1 += shift;
                }
                Orientation::Horizontal => {
                    edge.top += shift;
                    edge.bottom += shift;
                }
            }
            out.push(edge);
        }
    }
    out
}

/// Python's `sum()` of floats since 3.12: Neumaier's compensated summation.
fn python_sum(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut values = values.into_iter();
    let Some(first) = values.next() else {
        return 0.0;
    };
    let (mut high, mut low) = (first, 0.0_f64);
    for x in values {
        let t = high + x;
        if high.abs() >= x.abs() {
            low += (high - t) + x;
        } else {
            low += (x - t) + high;
        }
        high = t;
    }
    if low != 0.0 && low.is_finite() {
        high + low
    } else {
        high
    }
}

/// Joins edges of one line that overlap or lie within the join tolerance.
fn join(edges: &[Edge]) -> Vec<Edge> {
    let horizontal = edges[0].orientation != Some(Orientation::Vertical);
    let start = |e: &Edge| if horizontal { e.x0 } else { e.top };
    let end = |e: &Edge| if horizontal { e.x1 } else { e.bottom };
    let mut sorted = edges.to_vec();
    sorted.sort_by(|a, b| py_cmp(start(a), start(b)));
    let mut joined: Vec<Edge> = vec![sorted[0]];
    for edge in &sorted[1..] {
        let last = joined.last_mut().expect("starts with one edge");
        if start(edge) <= end(last) + JOIN_TOLERANCE {
            if end(edge) > end(last) {
                if horizontal {
                    last.x1 = edge.x1;
                    last.width = edge.x1 - last.x0;
                } else {
                    let grow = edge.bottom - last.bottom;
                    last.bottom = edge.bottom;
                    last.height += grow;
                }
            }
        } else {
            joined.push(*edge);
        }
    }
    joined
}

#[cfg(test)]
mod tests {
    use super::python_sum;

    #[test]
    fn sums_like_python() {
        // Python 3.12+: sum([0.1] * 10) == 1.0 and sum([1e16, 1.0, -1e16]) == 1.0.
        assert_eq!(python_sum([0.1; 10]), 1.0);
        assert_eq!(python_sum([1e16, 1.0, -1e16]), 1.0);
    }
}
