//! Matrices of ISO 32000-1 §8.3.4 (`[a b c d e f]`) and a page's geometry: its rotation, its media box and the
//! attributes it inherits from its parents.

// Matrices keep the `[a b c d e f]` names of the specification, so they read like it.
#![allow(clippy::many_single_char_names)]

use pdf_oxide::PdfDocument;
use pdf_oxide::object::Object;

pub(super) type Matrix = [f64; 6];

pub(super) const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// `m1` then `m0`.
pub(super) fn multiply(m1: Matrix, m0: Matrix) -> Matrix {
    let [a1, b1, c1, d1, e1, f1] = m1;
    let [a0, b0, c0, d0, e0, f0] = m0;
    [
        a0 * a1 + c0 * b1,
        b0 * a1 + d0 * b1,
        a0 * c1 + c0 * d1,
        b0 * c1 + d0 * d1,
        a0 * e1 + c0 * f1 + e0,
        b0 * e1 + d0 * f1 + f0,
    ]
}

pub(super) fn translate(m: Matrix, x: f64, y: f64) -> Matrix {
    let [a, b, c, d, e, f] = m;
    [a, b, c, d, x * a + y * c + e, x * b + y * d + f]
}

pub(super) fn apply(m: Matrix, x: f64, y: f64) -> (f64, f64) {
    let [a, b, c, d, e, f] = m;
    (a * x + c * y + e, b * x + d * y + f)
}

/// The smallest box holding every point, as pdfminer's `get_bound` computes it.
pub(super) fn bound(points: &[(f64, f64)]) -> (f64, f64, f64, f64) {
    let low = |acc: f64, v: f64| if v < acc { v } else { acc };
    let high = |acc: f64, v: f64| if v > acc { v } else { acc };
    points.iter().fold(
        (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY),
        |(x0, y0, x1, y1), &(x, y)| (low(x0, x), low(y0, y), high(x1, x), high(y1, y)),
    )
}

pub(super) fn rotation(document: &PdfDocument, page: &Object) -> i64 {
    inherited(document, page, "Rotate")
        .and_then(|o| o.as_integer())
        .unwrap_or(0)
        .rem_euclid(360)
}

/// A page attribute, looked up through `/Parent` when the page does not set it (ISO 32000-1 §7.7.3.4).
pub(super) fn inherited(document: &PdfDocument, page: &Object, key: &str) -> Option<Object> {
    let mut node = document.resolve_object(page).ok()?;
    for _ in 0..32 {
        let dict = node.as_dict()?;
        if let Some(value) = dict.get(key) {
            return document.resolve_object(value).ok();
        }
        node = document.resolve_object(dict.get("Parent")?).ok()?;
    }
    None
}

pub(super) fn number(object: &Object) -> Option<f64> {
    #[allow(clippy::cast_precision_loss)]
    object.as_real().or_else(|| object.as_integer().map(|i| i as f64))
}

/// The page's base transformation for its `/Rotate` and `/MediaBox`.
pub(super) fn page_matrix(document: &PdfDocument, page: &Object) -> Matrix {
    let rotate = rotation(document, page);
    let media: Vec<f64> = inherited(document, page, "MediaBox")
        .and_then(|o| o.as_array().map(|a| a.iter().filter_map(number).collect()))
        .unwrap_or_default();
    let [x0, y0, x1, y1] = <[f64; 4]>::try_from(media).unwrap_or([0.0, 0.0, 612.0, 792.0]);
    match rotate {
        90 => [0.0, -1.0, 1.0, 0.0, -y0, x1],
        180 => [-1.0, 0.0, 0.0, -1.0, x1, y1],
        270 => [0.0, 1.0, -1.0, 0.0, y1, -x0],
        _ => [1.0, 0.0, 0.0, 1.0, -x0, -y0],
    }
}
