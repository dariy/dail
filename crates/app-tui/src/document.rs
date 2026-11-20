//! A message body's [`Document`] as terminal rows (MISC-28b): paragraphs reflowed to the pane,
//! one blank row between blocks, headings bold, lists and quotes indented, and links underlined
//! with their number after them as `[n]`, bold and underlined (the link colour when the theme's
//! colours are on): a run of its own, never text, so a sender's own `[1]` cannot pass for one.
//! Quoted text and signatures ([`Block::Fold`]) are shown dimmed below a label: the TUI has no
//! fold toggle. Every span goes through [`clean`].

use app_core::document::{Block, Document, FoldKind, Span};
use app_front::i18n::Key;
use ratatui::style::{Modifier, Style};

use crate::text::{clean, s, width};
use crate::view::dim;

/// Text with one style.
pub type Run = (String, Style);

/// One logical row: its runs, and the runs that start each of its wrapped continuation rows
/// (a list item's indent, a quote's bar).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Row {
    /// The row's runs, its first-line prefix included.
    pub runs: Vec<Run>,
    /// What each wrapped continuation row starts with.
    pub hang: Vec<Run>,
}

impl Row {
    /// A row of one run.
    pub fn plain(text: String, style: Style) -> Self {
        Self {
            runs: vec![(text, style)],
            hang: Vec::new(),
        }
    }

    fn blank() -> Self {
        Self::default()
    }

    /// Puts `first` before the row's first line and `rest` before each other line.
    fn prefixed(mut self, first: &[Run], rest: &[Run]) -> Self {
        let mut runs = first.to_vec();
        runs.append(&mut self.runs);
        let mut hang = rest.to_vec();
        hang.append(&mut self.hang);
        Self { runs, hang }
    }
}

/// The rows of `doc`, not yet wrapped.
pub fn rows(doc: &Document) -> Vec<Row> {
    blocks(&doc.blocks, Style::default())
}

fn blocks(blocks: &[Block], base: Style) -> Vec<Row> {
    let mut out: Vec<Row> = Vec::new();
    for block in blocks {
        let rows = block_rows(block, base);
        if rows.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(Row::blank());
        }
        out.extend(rows);
    }
    out
}

fn block_rows(block: &Block, base: Style) -> Vec<Row> {
    match block {
        Block::Paragraph { spans } => span_rows(spans, base),
        Block::Heading { spans, .. } => span_rows(spans, base.add_modifier(Modifier::BOLD)),
        Block::List {
            ordered,
            start,
            items,
        } => {
            let mut out = Vec::new();
            for (n, item) in items.iter().enumerate() {
                let bullet = if *ordered {
                    let number = u64::from(*start) + u64::try_from(n).unwrap_or(0);
                    format!("{number}. ")
                } else {
                    "• ".to_owned()
                };
                let indent = " ".repeat(width(&bullet));
                for (i, row) in blocks(item, base).into_iter().enumerate() {
                    let first = if i == 0 { &bullet } else { &indent };
                    out.push(row.prefixed(&[(first.clone(), base)], &[(indent.clone(), base)]));
                }
            }
            out
        }
        Block::Quote { blocks: inner } => quoted(blocks(inner, base)),
        Block::Preformatted { text } => text
            .lines()
            .map(|line| Row::plain(clean(line), base))
            .collect(),
        Block::Table { rows } => rows
            .iter()
            .map(|cells| {
                let mut runs: Vec<Run> = Vec::new();
                for (i, cell) in cells.iter().enumerate() {
                    if i > 0 {
                        runs.push((" │ ".to_owned(), dim()));
                    }
                    let style = if cell.header {
                        base.add_modifier(Modifier::BOLD)
                    } else {
                        base
                    };
                    // A cell's line breaks become spaces: one table row is one row.
                    runs.extend(span_runs(&cell.spans, style).into_iter().flatten());
                }
                Row {
                    runs,
                    hang: Vec::new(),
                }
            })
            .collect(),
        Block::Rule => vec![Row::plain("─".repeat(24), dim())],
        Block::Fold {
            kind,
            blocks: inner,
        } => {
            let label = match kind {
                FoldKind::Quote => s(Key::QuotedText),
                FoldKind::Signature => s(Key::Signature),
            };
            let faded = base.add_modifier(Modifier::DIM);
            let body = blocks(inner, faded);
            let body = match kind {
                FoldKind::Quote => quoted(body),
                FoldKind::Signature => body,
            };
            let mut out = vec![Row::plain(format!("┄ {label}"), dim())];
            out.extend(body);
            out
        }
    }
}

/// `rows` behind a quote bar.
fn quoted(rows: Vec<Row>) -> Vec<Row> {
    let bar = vec![("│ ".to_owned(), dim())];
    rows.into_iter().map(|r| r.prefixed(&bar, &bar)).collect()
}

/// `spans` as rows, one per hard line break.
fn span_rows(spans: &[Span], base: Style) -> Vec<Row> {
    span_runs(spans, base)
        .into_iter()
        .map(|runs| Row {
            runs,
            hang: Vec::new(),
        })
        .collect()
}

/// The runs of `spans`, split at their line breaks, with a `[n]` marker run after each
/// numbered link.
fn span_runs(spans: &[Span], base: Style) -> Vec<Vec<Run>> {
    let mut lines: Vec<Vec<Run>> = vec![Vec::new()];
    for span in spans {
        let style = span_style(span, base);
        for (i, piece) in span.text.split('\n').enumerate() {
            if i > 0 {
                lines.push(Vec::new());
            }
            let text = clean(piece);
            if !text.is_empty()
                && let Some(line) = lines.last_mut()
            {
                line.push((text, style));
            }
        }
        if let Some(number) = span.note
            && let Some(line) = lines.last_mut()
        {
            line.push((format!("[{number}]"), marker_style(base)));
        }
    }
    lines
}

fn span_style(span: &Span, base: Style) -> Style {
    let mut style = base;
    if span.bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    if span.italic || span.image.is_some() {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if span.strike {
        style = style.add_modifier(Modifier::CROSSED_OUT);
    }
    if span.link.is_some() {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    style
}

/// A link marker's style: bold and underlined, the link colour.
pub fn marker_style(base: Style) -> Style {
    base.add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
}

/// `row` wrapped at word boundaries into lines of at most `columns` columns. A word is never
/// split from a run glued to it (a link and its marker); a word wider than a line is split.
pub fn wrap_row(row: &Row, columns: usize) -> Vec<Vec<Run>> {
    let columns = columns.max(1);
    // Words: pieces up to and including a space, across run boundaries.
    let mut words: Vec<Vec<Run>> = vec![Vec::new()];
    for (text, style) in &row.runs {
        for piece in text.split_inclusive(' ') {
            if let Some(word) = words.last_mut() {
                word.push((piece.to_owned(), *style));
            }
            if piece.ends_with(' ') {
                words.push(Vec::new());
            }
        }
    }
    let hang_width: usize = row.hang.iter().map(|(t, _)| width(t)).sum();
    let mut lines: Vec<Vec<Run>> = Vec::new();
    let mut line: Vec<Run> = Vec::new();
    // Columns used, and how many of them the line's hang takes.
    let (mut used, mut floor) = (0, 0);
    for word in words.into_iter().filter(|w| !w.is_empty()) {
        let full: usize = word.iter().map(|(t, _)| width(t)).sum();
        let spaces = word
            .last()
            .map_or(0, |(t, _)| width(t) - width(t.trim_end()));
        let shown = full - spaces;
        if used > floor && used + shown > columns {
            lines.push(trimmed(std::mem::replace(&mut line, row.hang.clone())));
            (used, floor) = (hang_width, hang_width);
        }
        if used + shown <= columns {
            used += full;
            line.extend(word);
            continue;
        }
        // Wider than a line: split it by characters.
        for (text, style) in word {
            for c in text.chars() {
                let cw = width(c.encode_utf8(&mut [0; 4]));
                if used + cw > columns && used > floor {
                    lines.push(trimmed(std::mem::replace(&mut line, row.hang.clone())));
                    (used, floor) = (hang_width, hang_width);
                }
                push_char(&mut line, c, style);
                used += cw;
            }
        }
    }
    lines.push(trimmed(line));
    lines
}

fn push_char(line: &mut Vec<Run>, c: char, style: Style) {
    match line.last_mut() {
        Some((text, s)) if *s == style => text.push(c),
        _ => line.push((c.to_string(), style)),
    }
}

/// `line` without its trailing spaces.
fn trimmed(mut line: Vec<Run>) -> Vec<Run> {
    while let Some((text, _)) = line.last_mut() {
        let kept = text.trim_end().len();
        text.truncate(kept);
        if text.is_empty() {
            line.pop();
        } else {
            break;
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(text: &str) -> Span {
        Span {
            text: text.into(),
            ..Span::default()
        }
    }

    fn link(text: &str, url: &str, note: u32) -> Span {
        Span {
            text: text.into(),
            link: Some(url.into()),
            note: Some(note),
            ..Span::default()
        }
    }

    fn texts(lines: &[Vec<Run>]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.iter().map(|(t, _)| t.as_str()).collect())
            .collect()
    }

    #[test]
    fn a_link_is_underlined_and_followed_by_its_own_marker_run() {
        let doc = Document {
            blocks: vec![Block::Paragraph {
                spans: vec![
                    span("See "),
                    link("activity", "https://x.test", 1),
                    span("."),
                ],
            }],
        };
        let rows = rows(&doc);
        assert_eq!(rows.len(), 1);
        let runs = &rows[0].runs;
        assert_eq!(runs[1].0, "activity");
        assert!(runs[1].1.add_modifier.contains(Modifier::UNDERLINED));
        assert_eq!(runs[2], ("[1]".to_owned(), marker_style(Style::default())));
        assert_eq!(runs[3].0, ".");
    }

    #[test]
    fn a_senders_own_bracket_number_is_plain_text() {
        let doc = Document {
            blocks: vec![Block::Paragraph {
                spans: vec![span("Fake [1]")],
            }],
        };
        let rows = rows(&doc);
        assert_eq!(
            rows[0].runs,
            vec![("Fake [1]".to_owned(), Style::default())]
        );
    }

    #[test]
    fn a_marker_stays_on_the_line_of_its_link() {
        let doc = Document {
            blocks: vec![Block::Paragraph {
                spans: vec![span("aaaa bbbb "), link("cccc", "https://x.test", 12)],
            }],
        };
        let lines = wrap_row(&rows(&doc)[0], 12);
        assert_eq!(texts(&lines), vec!["aaaa bbbb", "cccc[12]"]);
    }

    #[test]
    fn blocks_are_one_blank_row_apart_and_lists_hang() {
        let doc = Document {
            blocks: vec![
                Block::Heading {
                    level: 1,
                    spans: vec![span("Title")],
                },
                Block::List {
                    ordered: true,
                    start: 1,
                    items: vec![vec![Block::Paragraph {
                        spans: vec![span("one two three four")],
                    }]],
                },
            ],
        };
        let rows = rows(&doc);
        assert!(rows[0].runs[0].1.add_modifier.contains(Modifier::BOLD));
        assert!(rows[1].runs.is_empty());
        let lines = wrap_row(&rows[2], 10);
        assert_eq!(texts(&lines), vec!["1. one two", "   three", "   four"]);
    }

    #[test]
    fn folds_are_labelled_and_dimmed() {
        let doc = Document {
            blocks: vec![Block::Fold {
                kind: FoldKind::Quote,
                blocks: vec![Block::Paragraph {
                    spans: vec![span("earlier")],
                }],
            }],
        };
        let rows = rows(&doc);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].runs[0].0, "│ ");
        assert!(rows[1].runs[1].1.add_modifier.contains(Modifier::DIM));
    }
}
