//! The display form of a message body: the subset of HTML that dail renders, and plain text read as paragraphs,
//! headings and safe Markdown (MISC-28).
//!
//! `app_io::document` builds it from a message's HTML part, `app_io::plain` from its plain text, and the reader
//! draws it. It is email
//! content: front-ends only display it. Links are kept only for `http`, `https` and `mailto`
//! targets, images only as their alt text, and nothing in a document refers to anything remote
//! that would be loaded on display.

use serde::{Deserialize, Serialize};

/// An HTML body as blocks, in reading order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    /// The top-level blocks.
    pub blocks: Vec<Block>,
}

/// One block of a [`Document`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    /// A run of text (a `<p>`, a `<div>`'s text, …). `\n` is a line break.
    Paragraph {
        /// The text.
        spans: Vec<Span>,
    },
    /// `<h1>` … `<h6>`.
    Heading {
        /// 1 to 6.
        level: u8,
        /// The text.
        spans: Vec<Span>,
    },
    /// `<ul>` / `<ol>`.
    List {
        /// Numbered (`<ol>`).
        ordered: bool,
        /// The first item's number (`<ol start>`).
        start: u32,
        /// The items, each a list of blocks.
        items: Vec<Vec<Block>>,
    },
    /// `<blockquote>`.
    Quote {
        /// The quoted blocks.
        blocks: Vec<Block>,
    },
    /// `<pre>`: text with its spacing and line breaks kept, shown in a monospace font.
    Preformatted {
        /// The text.
        text: String,
    },
    /// A data table (layout tables are flattened into their cells' blocks).
    Table {
        /// The rows, each a list of cells.
        rows: Vec<Vec<Cell>>,
    },
    /// `<hr>`.
    Rule,
    /// Text the reader folds away until asked: the quoted earlier messages of a reply, or a
    /// signature. Not nested: a fold holds no other fold.
    Fold {
        /// What it holds.
        kind: FoldKind,
        /// The folded blocks.
        blocks: Vec<Block>,
    },
}

/// What a [`Block::Fold`] holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoldKind {
    /// The quoted text of earlier messages (Gmail's `gmail_quote`, `<blockquote type=cite>`,
    /// Outlook's reply header and what follows it).
    Quote,
    /// The sender's signature (`gmail_signature`, `moz-signature`, a `-- ` line).
    Signature,
}

/// One table cell.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    /// A header cell (`<th>`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub header: bool,
    /// The text.
    pub spans: Vec<Span>,
}

/// Text with one style.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    /// The text. Untrusted.
    pub text: String,
    /// `<b>`, `<strong>`, `<th>`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    /// `<i>`, `<em>`, and the alt text of an image.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    /// `<code>`, `<kbd>`, `<tt>`, …
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub code: bool,
    /// `<s>`, `<del>`, `<strike>`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub strike: bool,
    /// The target of the enclosing link: an `http`, `https` or `mailto` URL. Untrusted, and it
    /// may differ from what the text says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    /// An inline image of the message (`<img src="cid:…">`): its `Content-ID`. `text` is then
    /// the alt text (`[alt]`, or `[image]`), shown unless the user lets the sender's images show
    /// (`show_images_from`). Remote images are never kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// The number of this span's link in the document's *Links* list, on the last span of a link (MISC-28). The
    /// reader draws it as a marker of its own after the span (a raised number), never as text, so a sender's `(1)`
    /// or `[1]` cannot pass for one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<u32>,
}

/// One entry of a document's *Links* list (MISC-28).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkNote {
    /// Its marker's number, from 1.
    pub number: u32,
    /// The target. Untrusted.
    pub url: String,
    /// The link's text where it first appears (HTML, or a Markdown link's); empty for a bare URL. Untrusted.
    pub title: String,
}

impl Span {
    /// Whether `other` has the same style (and so can be merged into this span). Images are
    /// never merged.
    pub fn same_style(&self, other: &Self) -> bool {
        self.image.is_none()
            && other.image.is_none()
            && self.bold == other.bold
            && self.italic == other.italic
            && self.code == other.code
            && self.strike == other.strike
            && self.link == other.link
            && self.note.is_none()
            && other.note.is_none()
    }
}

impl Document {
    /// The `Content-ID`s of the document's inline images, in order of appearance, without
    /// duplicates.
    pub fn images(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for block in &self.blocks {
            block.visit_spans(&mut |span| {
                if let Some(image) = span.image.as_deref()
                    && !out.contains(&image)
                {
                    out.push(image);
                }
            });
        }
        out
    }

    /// The numbered links, in number order: what the *Links* list shows (MISC-28). Each number appears once, with
    /// the text of the first link that has it.
    pub fn notes(&self) -> Vec<LinkNote> {
        let mut out: Vec<LinkNote> = Vec::new();
        let mut run = String::new();
        for block in &self.blocks {
            block.visit_spans(&mut |span| {
                if span.link.is_some() {
                    run.push_str(&span.text);
                } else {
                    run.clear();
                }
                if let (Some(number), Some(url)) = (span.note, span.link.as_deref()) {
                    if !out.iter().any(|n| n.number == number) {
                        out.push(LinkNote {
                            number,
                            url: url.to_owned(),
                            title: run.trim().to_owned(),
                        });
                    }
                    run.clear();
                }
            });
            run.clear();
        }
        out.sort_by_key(|n| n.number);
        out
    }

    /// Numbers the links (MISC-28): each target gets a number at its first appearance, and the last span of every
    /// link carries it as [`Span::note`]. A link whose text is its own target (a bare URL) loses the text: the
    /// marker alone stands for it.
    pub fn number_links(&mut self) {
        let mut numbers: Vec<String> = Vec::new();
        for block in &mut self.blocks {
            block.number_links(&mut numbers);
        }
    }

    /// The links of the document, in order of appearance, without duplicates.
    pub fn links(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for block in &self.blocks {
            block.links(&mut out);
        }
        out
    }
}

impl Block {
    fn visit_spans<'a>(&'a self, f: &mut impl FnMut(&'a Span)) {
        match self {
            Self::Paragraph { spans } | Self::Heading { spans, .. } => spans.iter().for_each(f),
            Self::Table { rows } => rows.iter().flatten().flat_map(|c| &c.spans).for_each(f),
            Self::List { items, .. } => items.iter().flatten().for_each(|b| b.visit_spans(f)),
            Self::Quote { blocks } | Self::Fold { blocks, .. } => {
                blocks.iter().for_each(|b| b.visit_spans(f));
            }
            Self::Preformatted { .. } | Self::Rule => {}
        }
    }

    fn number_links(&mut self, numbers: &mut Vec<String>) {
        match self {
            Self::Paragraph { spans } | Self::Heading { spans, .. } => number_spans(spans, numbers),
            Self::Table { rows } => rows
                .iter_mut()
                .flatten()
                .for_each(|cell| number_spans(&mut cell.spans, numbers)),
            Self::List { items, .. } => items
                .iter_mut()
                .flatten()
                .for_each(|b| b.number_links(numbers)),
            Self::Quote { blocks } | Self::Fold { blocks, .. } => {
                blocks.iter_mut().for_each(|b| b.number_links(numbers));
            }
            Self::Preformatted { .. } | Self::Rule => {}
        }
    }

    fn links<'a>(&'a self, out: &mut Vec<&'a str>) {
        let mut add = |spans: &'a [Span]| {
            for link in spans.iter().filter_map(|s| s.link.as_deref()) {
                if !out.contains(&link) {
                    out.push(link);
                }
            }
        };
        match self {
            Self::Paragraph { spans } | Self::Heading { spans, .. } => add(spans),
            Self::Table { rows } => rows.iter().flatten().for_each(|cell| add(&cell.spans)),
            Self::List { items, .. } => items.iter().flatten().for_each(|b| b.links(out)),
            Self::Quote { blocks } | Self::Fold { blocks, .. } => {
                blocks.iter().for_each(|b| b.links(out));
            }
            Self::Preformatted { .. } | Self::Rule => {}
        }
    }
}

/// Numbers the link runs of `spans` (see [`Document::number_links`]).
fn number_spans(spans: &mut [Span], numbers: &mut Vec<String>) {
    let mut i = 0;
    while i < spans.len() {
        let Some(link) = spans[i].link.clone() else {
            i += 1;
            continue;
        };
        let mut end = i;
        while end + 1 < spans.len()
            && spans[end + 1].link.as_ref() == Some(&link)
            && spans[end + 1].image.is_none()
        {
            end += 1;
        }
        let number = match numbers.iter().position(|u| *u == link) {
            Some(at) => at + 1,
            None => {
                numbers.push(link.clone());
                numbers.len()
            }
        };
        let text: String = spans[i..=end].iter().map(|s| s.text.as_str()).collect();
        let bare = {
            let shown = text.trim();
            shown == link || link.strip_prefix("mailto:") == Some(shown)
        };
        if bare {
            for span in &mut spans[i..=end] {
                span.text.clear();
            }
        }
        spans[end].note = u32::try_from(number).ok();
        i = end + 1;
    }
}
