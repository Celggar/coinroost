//! Page text from positioned glyphs, with the word and line rules of pdfplumber's `extract_text` (default
//! settings, no layout): glyphs are grouped into lines whose tops lie within 3 points of each other, sorted
//! left to right, split into words at blank glyphs and at gaps wider than 3 points, and printed with one
//! space between words and one newline between lines.

/// A glyph in page space with the y axis pointing down (`top` < `bottom`).
#[derive(Clone, Debug, PartialEq)]
pub struct Glyph {
    pub text: String,
    pub x0: f64,
    pub x1: f64,
    pub top: f64,
    pub bottom: f64,
    /// Not rotated or mirrored.
    pub upright: bool,
}

const X_TOLERANCE: f64 = 3.0;
const Y_TOLERANCE: f64 = 3.0;

const LIGATURES: &[(&str, &str)] = &[
    ("ﬀ", "ff"),
    ("ﬃ", "ffi"),
    ("ﬄ", "ffl"),
    ("ﬁ", "fi"),
    ("ﬂ", "fl"),
    ("ﬆ", "st"),
    ("ﬅ", "st"),
];

struct Word<'a> {
    glyphs: Vec<&'a Glyph>,
    top: f64,
}

/// The text of one page.
pub fn page_text(glyphs: &[Glyph]) -> String {
    let mut words = Vec::new();
    let mut rest = glyphs;
    while let Some(first) = rest.first() {
        let run = rest.iter().take_while(|g| g.upright == first.upright).count();
        let (group, tail) = rest.split_at(run);
        for line in group_lines(group, first.upright) {
            words.extend(split_words(&line, first.upright));
        }
        rest = tail;
    }
    print_lines(&words)
}

/// Glyphs clustered into lines (by top when upright, by x0 when rotated), each sorted along the line.
fn group_lines(glyphs: &[Glyph], upright: bool) -> Vec<Vec<&Glyph>> {
    let key = |g: &Glyph| if upright { g.top } else { g.x0 };
    let tolerance = if upright { Y_TOLERANCE } else { X_TOLERANCE };
    let ids = cluster_ids(&glyphs.iter().map(key).collect::<Vec<_>>(), tolerance);
    let mut tagged: Vec<(usize, &Glyph)> = glyphs.iter().zip(ids).map(|(g, id)| (id, g)).collect();
    tagged.sort_by_key(|(id, _)| *id);
    let mut lines: Vec<Vec<&Glyph>> = Vec::new();
    let mut last = None;
    for (id, glyph) in tagged {
        if last != Some(id) {
            lines.push(Vec::new());
            last = Some(id);
        }
        lines.last_mut().expect("pushed above").push(glyph);
    }
    for line in &mut lines {
        if upright {
            line.sort_by(|a, b| a.x0.total_cmp(&b.x0));
        } else {
            line.sort_by(|a, b| a.top.total_cmp(&b.top).then(a.bottom.total_cmp(&b.bottom)));
        }
    }
    lines
}

/// For each value, the index of its cluster: distinct values sorted, a new cluster whenever a value is
/// more than `tolerance` above the previous one.
fn cluster_ids(values: &[f64], tolerance: f64) -> Vec<usize> {
    let mut distinct = values.to_vec();
    distinct.sort_by(f64::total_cmp);
    distinct.dedup();
    let mut bounds: Vec<(f64, usize)> = Vec::with_capacity(distinct.len());
    let mut cluster = 0;
    for (i, value) in distinct.iter().enumerate() {
        if i > 0 && *value > distinct[i - 1] + tolerance {
            cluster += 1;
        }
        bounds.push((*value, cluster));
    }
    values
        .iter()
        .map(|v| {
            let at = bounds.partition_point(|(b, _)| b.total_cmp(v).is_lt());
            bounds[at].1
        })
        .collect()
}

fn split_words<'a>(line: &[&'a Glyph], upright: bool) -> Vec<Word<'a>> {
    let mut words: Vec<Word<'a>> = Vec::new();
    let mut current: Vec<&'a Glyph> = Vec::new();
    for &glyph in line {
        if crate::text::strip(&glyph.text).is_empty() && !glyph.text.is_empty() {
            flush(&mut words, &mut current);
        } else if current
            .last()
            .is_some_and(|prev| starts_new_word(prev, glyph, upright))
        {
            flush(&mut words, &mut current);
            current.push(glyph);
        } else {
            current.push(glyph);
        }
    }
    flush(&mut words, &mut current);
    words
}

fn flush<'a>(words: &mut Vec<Word<'a>>, current: &mut Vec<&'a Glyph>) {
    if current.is_empty() {
        return;
    }
    let glyphs = std::mem::take(current);
    let top = glyphs.iter().map(|g| g.top).fold(f64::INFINITY, f64::min);
    words.push(Word { glyphs, top });
}

fn starts_new_word(prev: &Glyph, curr: &Glyph, upright: bool) -> bool {
    let (along_prev_start, along_prev_end, along_curr, across_prev, across_curr, gap, drift) = if upright {
        (
            prev.x0,
            prev.x1,
            curr.x0,
            prev.top,
            curr.top,
            X_TOLERANCE,
            Y_TOLERANCE,
        )
    } else {
        (
            prev.top,
            prev.bottom,
            curr.top,
            prev.x0,
            curr.x0,
            Y_TOLERANCE,
            X_TOLERANCE,
        )
    };
    along_curr < along_prev_start
        || along_curr > along_prev_end + gap
        || (across_curr - across_prev).abs() > drift
}

/// Words in order, a new line whenever the cluster of their top changes.
fn print_lines(words: &[Word<'_>]) -> String {
    let ids = cluster_ids(&words.iter().map(|w| w.top).collect::<Vec<_>>(), Y_TOLERANCE);
    let mut out = String::new();
    let mut previous = None;
    for (word, id) in words.iter().zip(ids) {
        match previous {
            None => {}
            Some(p) if p == id => out.push(' '),
            Some(_) => out.push('\n'),
        }
        previous = Some(id);
        for glyph in &word.glyphs {
            let text = LIGATURES
                .iter()
                .find(|(l, _)| *l == glyph.text)
                .map_or(glyph.text.as_str(), |(_, e)| e);
            out.push_str(text);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{Glyph, page_text};

    fn word(text: &str, x: f64, top: f64) -> Vec<Glyph> {
        text.chars()
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
    fn splits_words_at_gaps_and_blanks_and_lines_by_top() {
        let mut glyphs = word("PAGO", 40.0, 100.0);
        glyphs.extend(word("12.50", 62.9, 101.5));
        glyphs.extend(word("ACH X", 40.0, 112.0));
        glyphs.extend(word("ﬁN", 90.0, 112.0));
        assert_eq!(page_text(&glyphs), "PAGO12.50\nACH X fiN");
    }

    #[test]
    fn a_gap_over_three_points_starts_a_word() {
        let mut glyphs = word("A", 40.0, 100.0);
        glyphs.extend(word("B", 48.1, 100.0));
        assert_eq!(page_text(&glyphs), "A B");
    }
}
