//! The display form of an HTML body: the subset of HTML that dail renders.
//!
//! `app_io::document` builds it from a message's HTML part and the reader draws it. It is email
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
