//! Glyph boxes straight from the content stream, positioned the way pdfminer does (and therefore the way
//! pdfplumber sees them): no glyph is merged, reordered or invented.
//!
//! The PDF objects, fonts, widths and Unicode mappings come from `pdf_oxide`; the text positioning is ours.
//! Its own character extraction is not used because it inserts spaces where a `TJ` offset looks like a
//! word gap, and those spaces change the words a statement line is split into.
//!
//! Positioning rules (ISO 32000-1 §9.4, with pdfminer's choices where the spec leaves room):
//! - each glyph advances by `width × size × Tz`; character spacing `Tc` is added between glyphs of a
//!   string and after a `TJ` offset, word spacing `Tw` after code 32 in single-byte fonts;
//! - a glyph box runs from the origin along the advance, and from `descent + rise` to one em above it;
//! - `q`/`Q` save and restore the text state together with the graphics state;
//! - form `XObject`s are drawn with a fresh text state and their own resources.
//!
//! Positions are then expressed the way pdfplumber does: `top = (height - y1) + mediabox_top`, with the
//! page's `MediaBox` normalized for its rotation, and `x` shifted by the box's left side when it is not 0.

// Matrices keep the `[a b c d e f]` names of ISO 32000-1 §8.3.4, so they read like the specification.
#![allow(clippy::many_single_char_names)]

use std::collections::HashMap;

use pdf_oxide::PdfDocument;
use pdf_oxide::content::{Operator, TextElement, parse_content_stream};
use pdf_oxide::fonts::FontInfo;
use pdf_oxide::object::Object;

use super::Glyph;
use super::edges::{Shape, ShapeKind};
use super::paths::Path;

pub(super) type Matrix = [f64; 6];

const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
/// Forms inside forms deeper than this are not drawn (guards against reference cycles).
const MAX_FORM_DEPTH: usize = 8;

/// `m1` then `m0`.
fn multiply(m1: Matrix, m0: Matrix) -> Matrix {
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

fn translate(m: Matrix, x: f64, y: f64) -> Matrix {
    let [a, b, c, d, e, f] = m;
    [a, b, c, d, x * a + y * c + e, x * b + y * d + f]
}

pub(super) fn apply(m: Matrix, x: f64, y: f64) -> (f64, f64) {
    let [a, b, c, d, e, f] = m;
    (a * x + c * y + e, b * x + d * y + f)
}

#[derive(Clone)]
struct TextState {
    font: Option<String>,
    size: f64,
    char_space: f64,
    word_space: f64,
    scaling: f64,
    leading: f64,
    rise: f64,
    matrix: Matrix,
    line: (f64, f64),
}

impl TextState {
    fn new() -> TextState {
        TextState {
            font: None,
            size: 0.0,
            char_space: 0.0,
            word_space: 0.0,
            scaling: 100.0,
            leading: 0.0,
            rise: 0.0,
            matrix: IDENTITY,
            line: (0.0, 0.0),
        }
    }

    fn move_line(&mut self, tx: f64, ty: f64) {
        let [a, b, c, d, e, f] = self.matrix;
        self.matrix = [a, b, c, d, tx * a + ty * c + e, tx * b + ty * d + f];
        self.line = (0.0, 0.0);
    }
}

/// What one page draws: glyphs and painted paths, each in content-stream order.
pub struct PageContent {
    pub glyphs: Vec<Glyph>,
    pub shapes: Vec<Shape>,
}

/// Every glyph and painted path of one page, with the y axis pointing down.
pub fn page_content(document: &PdfDocument, page: usize) -> Result<PageContent, String> {
    let data = document.get_page_content_data(page).map_err(|e| e.to_string())?;
    let page = document.get_page(page).map_err(|e| e.to_string())?;
    let resources = inherited(document, &page, "Resources").unwrap_or(Object::Null);
    let mut painter = Painter {
        document,
        frame: Frame::of(document, &page),
        glyphs: Vec::new(),
        shapes: Vec::new(),
    };
    painter.run(&data, &resources, page_matrix(document, &page), 0)?;
    Ok(PageContent {
        glyphs: painter.glyphs,
        shapes: painter.shapes,
    })
}

/// Every glyph of one page, in content-stream order, with the y axis pointing down.
pub fn page_glyphs(document: &PdfDocument, page: usize) -> Result<Vec<Glyph>, String> {
    page_content(document, page).map(|content| content.glyphs)
}

/// pdfplumber's page space: its page height, and the top and left of its `MediaBox`.
pub(super) struct Frame {
    height: f64,
    top: f64,
    left: f64,
}

impl Frame {
    fn of(document: &PdfDocument, page: &Object) -> Frame {
        let media: Vec<f64> = inherited(document, page, "MediaBox")
            .and_then(|o| o.as_array().map(|a| a.iter().filter_map(number).collect()))
            .unwrap_or_default();
        let [a, b, c, d] = <[f64; 4]>::try_from(media).unwrap_or([0.0, 0.0, 612.0, 792.0]);
        let (x0, x1) = if c < a { (c, a) } else { (a, c) };
        let (y0, y1) = if d < b { (d, b) } else { (b, d) };
        let (x0, y0, y1) = if matches!(rotation(document, page), 90 | 270) {
            (y0, x0, x1)
        } else {
            (x0, y0, y1)
        };
        let media_height = y1 - y0;
        let (top, bottom) = (media_height - y1, media_height - y0);
        Frame {
            height: bottom - top,
            top,
            left: x0,
        }
    }

    /// A box in pdfminer's space (y up, origin at the page's corner) as `(x0, x1, top, bottom)`.
    fn flip(&self, (x0, y0, x1, y1): (f64, f64, f64, f64)) -> (f64, f64, f64, f64) {
        let (x0, x1) = if self.left == 0.0 {
            (x0, x1)
        } else {
            (x0 + self.left, x1 + self.left)
        };
        (
            x0,
            x1,
            (self.height - y1) + self.top,
            (self.height - y0) + self.top,
        )
    }

    /// A painted path through `points` (pdfminer's space), as pdfplumber records it.
    pub(super) fn shape(&self, kind: ShapeKind, points: &[(f64, f64)]) -> Shape {
        let (x0, y0, x1, y1) = bound(points);
        let (left, right, top, bottom) = self.flip((x0, y0, x1, y1));
        Shape {
            kind,
            x0: left,
            x1: right,
            top,
            bottom,
            width: x1 - x0,
            height: y1 - y0,
            points: points
                .iter()
                .map(|&(x, y)| (self.left + x, self.top + self.height - y))
                .collect(),
        }
    }
}

/// The smallest box holding every point, as pdfminer's `get_bound` computes it.
fn bound(points: &[(f64, f64)]) -> (f64, f64, f64, f64) {
    let low = |acc: f64, v: f64| if v < acc { v } else { acc };
    let high = |acc: f64, v: f64| if v > acc { v } else { acc };
    points.iter().fold(
        (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY),
        |(x0, y0, x1, y1), &(x, y)| (low(x0, x), low(y0, y), high(x1, x), high(y1, y)),
    )
}

fn rotation(document: &PdfDocument, page: &Object) -> i64 {
    inherited(document, page, "Rotate")
        .and_then(|o| o.as_integer())
        .unwrap_or(0)
        .rem_euclid(360)
}

/// A page attribute, looked up through `/Parent` when the page does not set it (ISO 32000-1 §7.7.3.4).
fn inherited(document: &PdfDocument, page: &Object, key: &str) -> Option<Object> {
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

fn number(object: &Object) -> Option<f64> {
    #[allow(clippy::cast_precision_loss)]
    object.as_real().or_else(|| object.as_integer().map(|i| i as f64))
}

/// The page's base transformation for its `/Rotate` and `/MediaBox`.
fn page_matrix(document: &PdfDocument, page: &Object) -> Matrix {
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

struct Painter<'a> {
    document: &'a PdfDocument,
    frame: Frame,
    glyphs: Vec<Glyph>,
    shapes: Vec<Shape>,
}

/// Fonts of one resource dictionary, loaded on first use.
type Fonts = HashMap<String, Option<FontInfo>>;

impl Painter<'_> {
    fn run(&mut self, data: &[u8], resources: &Object, base: Matrix, depth: usize) -> Result<(), String> {
        let operators = parse_content_stream(data).map_err(|e| e.to_string())?;
        let mut fonts = Fonts::new();
        let mut ctm = base;
        let mut text = TextState::new();
        let mut stack: Vec<(Matrix, TextState)> = Vec::new();
        let mut path = Path::default();
        for operator in operators {
            match operator {
                Operator::SaveState => stack.push((ctm, text.clone())),
                Operator::RestoreState => {
                    if let Some((saved_ctm, saved_text)) = stack.pop() {
                        (ctm, text) = (saved_ctm, saved_text);
                    }
                }
                Operator::Cm { a, b, c, d, e, f } => ctm = multiply(widen([a, b, c, d, e, f]), ctm),
                Operator::BeginText => (text.matrix, text.line) = (IDENTITY, (0.0, 0.0)),
                Operator::Do { name } if depth < MAX_FORM_DEPTH => self.form(&name, resources, ctm, depth)?,
                other => {
                    if !path.operate(&other, ctm, &self.frame, &mut self.shapes) {
                        self.text_operator(other, &mut text, (resources, &mut fonts), ctm);
                    }
                }
            }
        }
        Ok(())
    }

    fn text_operator(
        &mut self,
        op: Operator,
        text: &mut TextState,
        fonts: (&Object, &mut Fonts),
        ctm: Matrix,
    ) {
        let (resources, fonts) = fonts;
        match op {
            Operator::Tc { char_space } => text.char_space = f64::from(char_space),
            Operator::Tw { word_space } => text.word_space = f64::from(word_space),
            Operator::Tz { scale } => text.scaling = f64::from(scale),
            Operator::TL { leading } => text.leading = -f64::from(leading),
            Operator::Ts { rise } => text.rise = f64::from(rise),
            Operator::Tf { font, size } => {
                text.size = f64::from(size);
                if !fonts.contains_key(&font) {
                    let info = lookup(self.document, resources, "Font", &font)
                        .and_then(|object| FontInfo::from_dict(&object, self.document).ok());
                    fonts.insert(font.clone(), info);
                }
                text.font = Some(font);
            }
            Operator::Td { tx, ty } => text.move_line(f64::from(tx), f64::from(ty)),
            Operator::TD { tx, ty } => {
                text.move_line(f64::from(tx), f64::from(ty));
                text.leading = f64::from(ty);
            }
            Operator::Tm { a, b, c, d, e, f } => {
                text.matrix = widen([a, b, c, d, e, f]);
                text.line = (0.0, 0.0);
            }
            Operator::TStar => next_line(text),
            Operator::Tj { text: bytes } => self.show(&[TextElement::String(bytes)], text, fonts, ctm),
            Operator::TJ { array } => self.show(&array, text, fonts, ctm),
            Operator::Quote { text: bytes } => {
                next_line(text);
                self.show(&[TextElement::String(bytes)], text, fonts, ctm);
            }
            Operator::DoubleQuote {
                word_space,
                char_space,
                text: bytes,
            } => {
                text.word_space = f64::from(word_space);
                text.char_space = f64::from(char_space);
                self.show(&[TextElement::String(bytes)], text, fonts, ctm);
            }
            _ => {}
        }
    }

    fn show(&mut self, elements: &[TextElement], text: &mut TextState, fonts: &Fonts, ctm: Matrix) {
        let Some(Some(font)) = text.font.as_ref().and_then(|name| fonts.get(name)) else {
            return;
        };
        let matrix = multiply(text.matrix, ctm);
        let scaling = text.scaling * 0.01;
        let multibyte = font.subtype == "Type0";
        let char_space = text.char_space * scaling;
        let word_space = if multibyte { 0.0 } else { text.word_space * scaling };
        let (mut x, y) = text.line;
        let mut need_char_space = false;
        let mut glyphs = Vec::new();
        for element in elements {
            match element {
                TextElement::Offset(offset) => {
                    x -= f64::from(*offset) * 0.001 * text.size * scaling;
                    need_char_space = true;
                }
                TextElement::String(bytes) => {
                    for code in codes(bytes, multibyte) {
                        if need_char_space {
                            x += char_space;
                        }
                        let (item, advance) = glyph(font, code, translate(matrix, x, y), text, scaling);
                        x += advance;
                        glyphs.push(self.frame.glyph(item));
                        if code == 32 && word_space != 0.0 {
                            x += word_space;
                        }
                        need_char_space = true;
                    }
                }
            }
        }
        text.line = (x, y);
        self.glyphs.extend(glyphs);
    }

    fn form(&mut self, name: &str, resources: &Object, ctm: Matrix, depth: usize) -> Result<(), String> {
        let Some(form) = lookup(self.document, resources, "XObject", name) else {
            return Ok(());
        };
        let Object::Stream { dict, .. } = &form else {
            return Ok(());
        };
        if dict.get("Subtype").and_then(Object::as_name) != Some("Form") || !dict.contains_key("BBox") {
            return Ok(());
        }
        let resolve = |key: &str| dict.get(key).and_then(|o| self.document.resolve_object(o).ok());
        let matrix = resolve("Matrix")
            .and_then(|o| {
                let values: Vec<f64> = o.as_array()?.iter().filter_map(number).collect();
                <[f64; 6]>::try_from(values).ok()
            })
            .unwrap_or(IDENTITY);
        let own_resources = resolve("Resources").unwrap_or_else(|| resources.clone());
        let data = form.decode_stream_data().map_err(|e| e.to_string())?;
        self.run(&data, &own_resources, multiply(matrix, ctm), depth + 1)
    }
}

fn next_line(text: &mut TextState) {
    let [a, b, c, d, e, f] = text.matrix;
    text.matrix = [a, b, c, d, text.leading * c + e, text.leading * d + f];
    text.line = (0.0, 0.0);
}

fn widen(m: [f32; 6]) -> Matrix {
    m.map(f64::from)
}

/// Character codes of a string: one byte each, or two bytes (big-endian) for composite fonts.
fn codes(bytes: &[u8], multibyte: bool) -> Vec<u32> {
    if multibyte {
        bytes
            .chunks(2)
            .map(|pair| pair.iter().fold(0u32, |code, &b| (code << 8) | u32::from(b)))
            .collect()
    } else {
        bytes.iter().map(|&b| u32::from(b)).collect()
    }
}

/// A glyph in pdfminer's space (y up).
struct RawGlyph {
    text: String,
    bbox: (f64, f64, f64, f64),
    upright: bool,
}

impl Frame {
    fn glyph(&self, raw: RawGlyph) -> Glyph {
        let (x0, x1, top, bottom) = self.flip(raw.bbox);
        Glyph {
            text: raw.text,
            x0,
            x1,
            top,
            bottom,
            upright: raw.upright,
        }
    }
}

/// The glyph for one code, and its advance along the line.
fn glyph(font: &FontInfo, code: u32, matrix: Matrix, text: &TextState, scaling: f64) -> (RawGlyph, f64) {
    let width = f64::from(font.get_glyph_width(u16::try_from(code).unwrap_or(u16::MAX))) * 0.001;
    let advance = width * text.size * scaling;
    let descent = f64::from(font.descent) * text.size;
    let bottom = descent + text.rise;
    let corners = [
        apply(matrix, 0.0, bottom),
        apply(matrix, advance, bottom),
        apply(matrix, advance, bottom + text.size),
        apply(matrix, 0.0, bottom + text.size),
    ];
    let x0 = corners.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
    let x1 = corners.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
    let y0 = corners.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let y1 = corners.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
    let [a, b, c, d, _, _] = matrix;
    let text_value = font
        .char_to_unicode(code)
        .unwrap_or_else(|| format!("(cid:{code})"));
    let glyph = RawGlyph {
        text: text_value,
        bbox: (x0, y0, x1, y1),
        upright: a * d * scaling > 0.0 && b * c <= 0.0,
    };
    (glyph, advance)
}

/// A named entry of a resource category (`Font`, `XObject`), references resolved.
fn lookup(document: &PdfDocument, resources: &Object, category: &str, name: &str) -> Option<Object> {
    let resources = document.resolve_object(resources).ok()?;
    let category = document
        .resolve_object(resources.as_dict()?.get(category)?)
        .ok()?;
    document.resolve_object(category.as_dict()?.get(name)?).ok()
}
