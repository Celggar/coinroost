//! Painted paths, recognized the way pdfminer recognizes them (and therefore the way pdfplumber sees its
//! lines, rectangles and curves).
//!
//! - Path construction is recorded as written; the current transformation applies when the path is
//!   painted. `re` is recorded as `m l l l h`. `n` discards the path; `q`/`Q` do not touch it.
//! - A painted path with several `m` is painted as one path per `m` and the segments that follow it; a
//!   path that does not start with `m` is not painted at all.
//! - Each segment contributes its end point (`h`: the subpath's start). A closing `l` back to the start
//!   right before `h` is dropped.
//! - `m l` and `m l h` are a line; `m l l l h` and `m l l l l` that close on the start with axis-aligned
//!   sides are a rectangle; anything else is a curve through those points.
//!
//! Known difference: the obsolete `F` operator is read as `f`. pdfminer neither paints nor discards the
//! path on `F`.

// Positions are compared exactly where the reference compares them exactly (see above).
#![allow(clippy::float_cmp)]

use pdf_oxide::content::Operator;

use super::edges::{Shape, ShapeKind};
use super::geometry::{Matrix, apply};
use super::glyphs::Frame;

struct Segment {
    op: u8,
    points: Vec<(f64, f64)>,
}

/// The path under construction in one content stream.
#[derive(Default)]
pub(super) struct Path {
    segments: Vec<Segment>,
}

impl Path {
    /// Handles a path construction or painting operator, adding what it paints to `shapes`. Returns
    /// `false` for any other operator.
    pub(super) fn operate(
        &mut self,
        op: &Operator,
        ctm: Matrix,
        frame: &Frame,
        shapes: &mut Vec<Shape>,
    ) -> bool {
        let w = |v: &f32| f64::from(*v);
        match op {
            Operator::MoveTo { x, y } => self.push(b'm', vec![(w(x), w(y))]),
            Operator::LineTo { x, y } => self.push(b'l', vec![(w(x), w(y))]),
            Operator::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x3,
                y3,
            } => self.push(b'c', vec![(w(x1), w(y1)), (w(x2), w(y2)), (w(x3), w(y3))]),
            Operator::CurveToV { x2, y2, x3, y3 } => self.push(b'v', vec![(w(x2), w(y2)), (w(x3), w(y3))]),
            Operator::CurveToY { x1, y1, x3, y3 } => self.push(b'y', vec![(w(x1), w(y1)), (w(x3), w(y3))]),
            Operator::ClosePath => self.push(b'h', Vec::new()),
            Operator::Rectangle { x, y, width, height } => {
                let (x, y, wd, ht) = (w(x), w(y), w(width), w(height));
                self.push(b'm', vec![(x, y)]);
                self.push(b'l', vec![(x + wd, y)]);
                self.push(b'l', vec![(x + wd, y + ht)]);
                self.push(b'l', vec![(x, y + ht)]);
                self.push(b'h', Vec::new());
            }
            Operator::Stroke
            | Operator::Fill
            | Operator::FillEvenOdd
            | Operator::FillStroke
            | Operator::FillStrokeEvenOdd => self.paint(ctm, frame, shapes),
            Operator::CloseAndStroke | Operator::CloseFillStroke | Operator::CloseFillStrokeEvenOdd => {
                self.push(b'h', Vec::new());
                self.paint(ctm, frame, shapes);
            }
            Operator::EndPath => self.segments.clear(),
            _ => return false,
        }
        true
    }

    fn push(&mut self, op: u8, points: Vec<(f64, f64)>) {
        self.segments.push(Segment { op, points });
    }

    fn paint(&mut self, ctm: Matrix, frame: &Frame, shapes: &mut Vec<Shape>) {
        let segments = std::mem::take(&mut self.segments);
        paint(&segments, ctm, frame, shapes);
    }
}

fn paint(path: &[Segment], ctm: Matrix, frame: &Frame, shapes: &mut Vec<Shape>) {
    if path.first().is_none_or(|s| s.op != b'm') {
        return;
    }
    let moves = path.iter().filter(|s| s.op == b'm').count();
    if moves > 1 {
        // Each `m` with the non-`m` segments after it; an `m` followed directly by another is skipped.
        for (start, _) in path.iter().enumerate().filter(|(_, s)| s.op == b'm') {
            let length = path[start + 1..].iter().take_while(|s| s.op != b'm').count();
            if length > 0 {
                paint(&path[start..=start + length], ctm, frame, shapes);
            }
        }
        return;
    }
    let start = path[0].points[0];
    let mut points: Vec<(f64, f64)> = path
        .iter()
        .map(|s| {
            let (x, y) = s.points.last().copied().unwrap_or(start);
            apply(ctm, x, y)
        })
        .collect();
    let mut shape: Vec<u8> = path.iter().map(|s| s.op).collect();
    if shape.len() > 3 && shape.ends_with(b"lh") && points[points.len() - 2] == points[0] {
        shape.truncate(shape.len() - 2);
        shape.push(b'h');
        points.pop();
    }
    let built = match shape.as_slice() {
        b"ml" | b"mlh" => frame.shape(ShapeKind::Line, &points[..2]),
        b"mlllh" | b"mllll" if is_rectangle(&points) => {
            let ((x0, y0), (x1, y1)) = (points[0], points[2]);
            frame.shape(ShapeKind::Rect, &[(x0, y0), (x1, y0), (x1, y1), (x0, y1)])
        }
        _ => frame.shape(ShapeKind::Curve, &points),
    };
    shapes.push(built);
}

/// Five points that close on the first, with sides along the axes.
fn is_rectangle(points: &[(f64, f64)]) -> bool {
    let Ok([(x0, y0), (x1, y1), (x2, y2), (x3, y3), last]) = <[(f64, f64); 5]>::try_from(points) else {
        return false;
    };
    let closed = (x0, y0) == last;
    closed
        && ((x0 == x1 && y1 == y2 && x2 == x3 && y3 == y0) || (y0 == y1 && x1 == x2 && y2 == y3 && x3 == x0))
}
