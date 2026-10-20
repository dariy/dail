//! HTML bodies as [`Document`]s: the subset of HTML the reader renders (`ARCHITECTURE.md §6`).
//!
//! The HTML is parsed with html5ever into a small DOM and walked once. Paragraphs, headings,
//! lists, block quotes, preformatted text, rules, data tables and inline styles (bold, italic,
//! code, strike-through, links) are kept; everything else is reduced to its text. Scripts,
//! styles, forms, frames, media and hidden elements are dropped. Images become their alt text,
//! links keep only `http`, `https` and `mailto` targets, and nothing is fetched. Layout tables
//! (cells holding more than one paragraph, nested tables, one column, `role="presentation"`)
//! are flattened into their cells' blocks. Quoted earlier messages and signatures, as mail
//! clients mark them, become [`Block::Fold`]s.

use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

use app_core::document::{Block, Cell, Document, FoldKind, Span};
use html5ever::interface::tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::{Attribute, QualName, local_name, ns, parse_document};

/// Elements nested deeper than this are reduced to their text (the walk is recursive).
const MAX_DEPTH: usize = 48;
/// Tables with more columns than this are flattened: they don't fit the reader.
const MAX_COLUMNS: usize = 8;
/// A `-- ` signature longer than this many lines is not folded (it is probably not one).
const MAX_SIGNATURE_LINES: usize = 15;

/// Parses an HTML body into the blocks the reader shows.
pub fn html_document(html: &str) -> Document {
    let dom = parse_document(Sink::default(), Default::default()).one(html);
    let mut flow = Flow::default();
    walk(&dom, &mut flow, &Span::default(), 0);
    free(&dom);
    let mut blocks = unnest_folds(flow.finish(), false);
    fold_signature(&mut blocks);
    Document { blocks }
}

/// Keeps folds at their outermost level: inside a fold, a quote fold becomes a plain quote
/// and a signature fold just its blocks.
fn unnest_folds(blocks: Vec<Block>, inside: bool) -> Vec<Block> {
    let mut out = Vec::with_capacity(blocks.len());
    for block in blocks {
        match block {
            Block::Fold { kind, blocks } => {
                let blocks = unnest_folds(blocks, true);
                match (inside, kind) {
                    (false, kind) => out.push(Block::Fold { kind, blocks }),
                    (true, FoldKind::Quote) => out.push(Block::Quote { blocks }),
                    (true, FoldKind::Signature) => out.extend(blocks),
                }
            }
            Block::Quote { blocks } => out.push(Block::Quote {
                blocks: unnest_folds(blocks, inside),
            }),
            Block::List {
                ordered,
                start,
                items,
            } => out.push(Block::List {
                ordered,
                start,
                items: items
                    .into_iter()
                    .map(|item| unnest_folds(item, inside))
                    .collect(),
            }),
            other => out.push(other),
        }
    }
    out
}

/// Folds a `-- ` signature: a top-level paragraph starting with a `--` line, and the blocks after
/// it up to the next fold, if they are short.
fn fold_signature(blocks: &mut Vec<Block>) {
    let starts_signature = |block: &Block| match block {
        Block::Paragraph { spans } => {
            let text: String = spans.iter().map(|s| s.text.as_str()).collect();
            text == "--" || text.starts_with("--\n")
        }
        _ => false,
    };
    let marked = blocks.iter().position(|b| {
        matches!(
            b,
            Block::Fold {
                kind: FoldKind::Signature,
                ..
            }
        )
    });
    if let Some(at) = marked {
        // Gmail puts the `-- ` line just before its signature block.
        if at > 0 && starts_signature(&blocks[at - 1]) {
            let line = blocks.remove(at - 1);
            if let Some(Block::Fold { blocks: inner, .. }) = blocks.get_mut(at - 1) {
                inner.insert(0, line);
            }
        }
        return;
    }
    let Some(start) = blocks.iter().position(starts_signature) else {
        return;
    };
    let end = blocks
        .iter()
        .skip(start)
        .position(|b| matches!(b, Block::Fold { .. }))
        .map_or(blocks.len(), |n| start + n);
    let lines: usize = blocks[start..end]
        .iter()
        .map(|b| match b {
            Block::Paragraph { spans } => {
                1 + spans
                    .iter()
                    .map(|s| s.text.matches('\n').count())
                    .sum::<usize>()
            }
            _ => 1,
        })
        .sum();
    if start == 0 || lines > MAX_SIGNATURE_LINES {
        return;
    }
    let signature: Vec<Block> = blocks.drain(start..end).collect();
    blocks.insert(
        start,
        Block::Fold {
            kind: FoldKind::Signature,
            blocks: signature,
        },
    );
}

/// The fold an element starts, as mail clients mark quoted text and signatures.
fn fold_kind(name: &str, attrs: &[Attribute]) -> Option<FoldKind> {
    let classes = attr(attrs, "class").unwrap_or_default();
    let has_class = |wanted: &str| classes.split_ascii_whitespace().any(|c| c == wanted);
    if has_class("gmail_signature")
        || has_class("moz-signature")
        || attr(attrs, "data-smartmail") == Some("gmail_signature")
    {
        return Some(FoldKind::Signature);
    }
    let cite =
        name == "blockquote" && attr(attrs, "type").is_some_and(|t| t.eq_ignore_ascii_case("cite"));
    (cite || has_class("gmail_quote") || has_class("yahoo_quoted")).then_some(FoldKind::Quote)
}

/// Outlook's reply header (`divRplyFwdMsg`, or the `appendonsend` marker before it): it and
/// everything after it is the quoted message.
fn is_outlook_reply(node: &Handle) -> bool {
    match &node.data {
        Data::Element { attrs, .. } => attr(&attrs.borrow(), "id")
            .is_some_and(|id| id == "divRplyFwdMsg" || id == "appendonsend"),
        _ => false,
    }
}

/// Takes the tree apart without recursion: dropping a deeply nested `Rc` tree would recurse
/// once per level.
fn free(root: &Handle) {
    let mut stack = vec![Rc::clone(root)];
    while let Some(node) = stack.pop() {
        stack.append(&mut node.children.borrow_mut());
        if let Data::Element {
            template: Some(contents),
            ..
        } = &node.data
        {
            stack.push(Rc::clone(contents));
        }
    }
}

// --- DOM -----------------------------------------------------------------------------------

type Handle = Rc<Node>;

#[derive(Debug)]
struct Node {
    parent: RefCell<Weak<Node>>,
    children: RefCell<Vec<Handle>>,
    data: Data,
}

#[derive(Debug)]
enum Data {
    Document,
    Text(RefCell<StrTendril>),
    Element {
        name: QualName,
        attrs: RefCell<Vec<Attribute>>,
        template: Option<Handle>,
    },
    /// Comments, processing instructions, doctypes: never shown.
    Other,
}

impl Node {
    fn new(data: Data) -> Handle {
        Rc::new(Self {
            parent: RefCell::new(Weak::new()),
            children: RefCell::new(Vec::new()),
            data,
        })
    }
}

fn detach(node: &Handle) {
    let parent = node.parent.replace(Weak::new()).upgrade();
    if let Some(parent) = parent {
        parent
            .children
            .borrow_mut()
            .retain(|child| !Rc::ptr_eq(child, node));
    }
}

fn append_child(parent: &Handle, child: NodeOrText<Handle>) {
    match child {
        NodeOrText::AppendText(text) => {
            if let Some(last) = parent.children.borrow().last()
                && let Data::Text(contents) = &last.data
            {
                contents.borrow_mut().push_tendril(&text);
                return;
            }
            let node = Node::new(Data::Text(RefCell::new(text)));
            *node.parent.borrow_mut() = Rc::downgrade(parent);
            parent.children.borrow_mut().push(node);
        }
        NodeOrText::AppendNode(node) => {
            detach(&node);
            *node.parent.borrow_mut() = Rc::downgrade(parent);
            parent.children.borrow_mut().push(node);
        }
    }
}

/// Builds the DOM for html5ever's tree builder.
struct Sink {
    document: Handle,
    /// The name returned for a node that isn't an element (the tree builder only asks for
    /// elements' names).
    no_name: QualName,
}

impl Default for Sink {
    fn default() -> Self {
        Self {
            document: Node::new(Data::Document),
            no_name: QualName::new(None, ns!(), local_name!("")),
        }
    }
}

impl TreeSink for Sink {
    type Handle = Handle;
    type Output = Handle;
    type ElemName<'a>
        = &'a QualName
    where
        Self: 'a;

    fn finish(self) -> Handle {
        self.document
    }

    fn parse_error(&self, _msg: Cow<'static, str>) {}

    fn get_document(&self) -> Handle {
        Rc::clone(&self.document)
    }

    fn elem_name<'a>(&'a self, target: &'a Handle) -> &'a QualName {
        match &target.data {
            Data::Element { name, .. } => name,
            _ => &self.no_name,
        }
    }

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> Handle {
        Node::new(Data::Element {
            name,
            attrs: RefCell::new(attrs),
            template: flags.template.then(|| Node::new(Data::Document)),
        })
    }

    fn create_comment(&self, _text: StrTendril) -> Handle {
        Node::new(Data::Other)
    }

    fn create_pi(&self, _target: StrTendril, _data: StrTendril) -> Handle {
        Node::new(Data::Other)
    }

    fn append(&self, parent: &Handle, child: NodeOrText<Handle>) {
        append_child(parent, child);
    }

    fn append_based_on_parent_node(
        &self,
        element: &Handle,
        prev_element: &Handle,
        child: NodeOrText<Handle>,
    ) {
        if element.parent.borrow().upgrade().is_some() {
            self.append_before_sibling(element, child);
        } else {
            append_child(prev_element, child);
        }
    }

    fn append_doctype_to_document(&self, _: StrTendril, _: StrTendril, _: StrTendril) {}

    fn get_template_contents(&self, target: &Handle) -> Handle {
        match &target.data {
            Data::Element {
                template: Some(contents),
                ..
            } => Rc::clone(contents),
            _ => Rc::clone(target),
        }
    }

    fn same_node(&self, x: &Handle, y: &Handle) -> bool {
        Rc::ptr_eq(x, y)
    }

    fn set_quirks_mode(&self, _mode: QuirksMode) {}

    fn append_before_sibling(&self, sibling: &Handle, child: NodeOrText<Handle>) {
        let Some(parent) = sibling.parent.borrow().upgrade() else {
            return;
        };
        let mut children = parent.children.borrow_mut();
        let Some(at) = children.iter().position(|c| Rc::ptr_eq(c, sibling)) else {
            return;
        };
        let node = match child {
            NodeOrText::AppendText(text) => {
                if let Some(previous) = at.checked_sub(1).and_then(|i| children.get(i))
                    && let Data::Text(contents) = &previous.data
                {
                    contents.borrow_mut().push_tendril(&text);
                    return;
                }
                Node::new(Data::Text(RefCell::new(text)))
            }
            NodeOrText::AppendNode(node) => {
                drop(children);
                detach(&node);
                children = parent.children.borrow_mut();
                node
            }
        };
        let at = children
            .iter()
            .position(|c| Rc::ptr_eq(c, sibling))
            .unwrap_or(children.len());
        *node.parent.borrow_mut() = Rc::downgrade(&parent);
        children.insert(at, node);
    }

    fn add_attrs_if_missing(&self, target: &Handle, attrs: Vec<Attribute>) {
        if let Data::Element {
            attrs: existing, ..
        } = &target.data
        {
            let mut existing = existing.borrow_mut();
            for attr in attrs {
                if !existing.iter().any(|e| e.name == attr.name) {
                    existing.push(attr);
                }
            }
        }
    }

    fn remove_from_parent(&self, target: &Handle) {
        detach(target);
    }

    fn reparent_children(&self, node: &Handle, new_parent: &Handle) {
        let children = std::mem::take(&mut *node.children.borrow_mut());
        for child in children {
            *child.parent.borrow_mut() = Rc::downgrade(new_parent);
            new_parent.children.borrow_mut().push(child);
        }
    }
}

// --- Blocks --------------------------------------------------------------------------------

/// The blocks of one container and the paragraph being collected.
#[derive(Default)]
struct Flow {
    blocks: Vec<Block>,
    spans: Vec<Span>,
}

impl Flow {
    /// Adds text, collapsing white space the way a browser does.
    fn text(&mut self, text: &str, style: &Span) {
        let mut out = String::new();
        let mut space = self.ends_in_space();
        for c in text.chars() {
            if is_html_space(c) {
                if !space {
                    out.push(' ');
                    space = true;
                }
            } else {
                out.push(c);
                space = false;
            }
        }
        self.push(out, style);
    }

    /// Adds text as it is (a line break).
    fn push(&mut self, text: String, style: &Span) {
        if text.is_empty() {
            return;
        }
        match self.spans.last_mut() {
            Some(last) if last.same_style(style) => last.text.push_str(&text),
            _ => self.spans.push(Span {
                text,
                ..style.clone()
            }),
        }
    }

    fn line_break(&mut self, style: &Span) {
        // Spaces before a line break are not shown.
        if let Some(last) = self.spans.last_mut() {
            let kept = last.text.trim_end_matches(' ').len();
            last.text.truncate(kept);
        }
        self.push("\n".to_owned(), style);
    }

    fn ends_in_space(&self) -> bool {
        self.spans
            .iter()
            .rev()
            .find_map(|s| s.text.chars().next_back())
            .is_none_or(|c| c == ' ' || c == '\n')
    }

    /// Ends the paragraph being collected.
    fn flush(&mut self) {
        let mut spans = std::mem::take(&mut self.spans);
        while let Some(last) = spans.last_mut() {
            let kept = last.text.trim_end_matches([' ', '\n']).len();
            last.text.truncate(kept);
            if last.text.is_empty() && last.image.is_none() {
                spans.pop();
            } else {
                break;
            }
        }
        if spans
            .iter()
            .any(|s| s.image.is_some() || is_visible(&s.text))
        {
            self.blocks.push(Block::Paragraph { spans });
        }
    }

    fn block(&mut self, block: Block) {
        self.flush();
        self.blocks.push(block);
    }

    fn finish(mut self) -> Vec<Block> {
        self.flush();
        self.blocks
    }
}

/// White space that HTML collapses (not the no-break space).
fn is_html_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0c')
}

/// Whether text shows anything: newsletters pad their hidden preview text with zero-width and
/// combining characters.
fn is_visible(text: &str) -> bool {
    text.chars().any(|c| {
        !c.is_whitespace()
            && !matches!(
                c,
                '\u{200b}'
                    | '\u{200c}'
                    | '\u{200d}'
                    | '\u{2060}'
                    | '\u{feff}'
                    | '\u{34f}'
                    | '\u{ad}'
            )
    })
}

fn walk(node: &Handle, flow: &mut Flow, style: &Span, depth: usize) {
    let children = node.children.borrow().clone();
    for (i, child) in children.iter().enumerate() {
        if depth < MAX_DEPTH && is_outlook_reply(child) {
            let mut rest = Flow::default();
            for sibling in children.iter().skip(i) {
                walk_one(sibling, &mut rest, style, depth);
            }
            let blocks = rest.finish();
            if blocks.is_empty() {
                flow.flush();
            } else {
                flow.block(Block::Fold {
                    kind: FoldKind::Quote,
                    blocks,
                });
            }
            return;
        }
        walk_one(child, flow, style, depth);
    }
}

fn walk_one(child: &Handle, flow: &mut Flow, style: &Span, depth: usize) {
    match &child.data {
        Data::Text(text) => flow.text(&text.borrow(), style),
        Data::Element { name, attrs, .. } => {
            if depth >= MAX_DEPTH {
                flow.text(&text_content(child), style);
            } else if !is_hidden(&attrs.borrow()) {
                element(child, &name.local, &attrs.borrow(), flow, style, depth + 1);
            }
        }
        Data::Document | Data::Other => {}
    }
}

fn blocks_of(node: &Handle, style: &Span, depth: usize) -> Vec<Block> {
    let mut flow = Flow::default();
    walk(node, &mut flow, style, depth);
    flow.finish()
}

fn element(
    node: &Handle,
    name: &str,
    attrs: &[Attribute],
    flow: &mut Flow,
    style: &Span,
    depth: usize,
) {
    let styled = |change: fn(&mut Span)| {
        let mut style = style.clone();
        change(&mut style);
        style
    };
    if let Some(kind) = fold_kind(name, attrs) {
        let blocks = blocks_of(node, style, depth);
        if blocks.is_empty() {
            flow.flush();
        } else {
            flow.block(Block::Fold { kind, blocks });
        }
        return;
    }
    match name {
        "head" | "script" | "style" | "template" | "title" | "meta" | "link" | "base"
        | "noscript" | "iframe" | "frame" | "frameset" | "object" | "embed" | "applet" | "svg"
        | "math" | "input" | "select" | "textarea" | "option" | "video" | "audio" | "source"
        | "track" | "canvas" | "map" | "area" | "picture" => {}
        "br" => flow.line_break(style),
        "img" => {
            let alt = attr(attrs, "alt").map(str::trim).unwrap_or_default();
            let mut italic = styled(|s| s.italic = true);
            italic.image = attr(attrs, "src").and_then(content_id);
            if italic.image.is_some() {
                let text = if is_visible(alt) {
                    format!("[{alt}]")
                } else {
                    "[image]".to_owned()
                };
                flow.push(text, &italic);
            } else if is_visible(alt) {
                flow.text(&format!("[{alt}]"), &italic);
            }
        }
        "a" => {
            let mut linked = style.clone();
            linked.link = attr(attrs, "href").and_then(safe_link);
            walk(node, flow, &linked, depth);
        }
        "b" | "strong" => walk(node, flow, &styled(|s| s.bold = true), depth),
        "i" | "em" | "cite" | "var" | "dfn" => {
            walk(node, flow, &styled(|s| s.italic = true), depth)
        }
        "code" | "kbd" | "samp" | "tt" => walk(node, flow, &styled(|s| s.code = true), depth),
        "s" | "strike" | "del" => walk(node, flow, &styled(|s| s.strike = true), depth),
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let level = name.get(1..).and_then(|n| n.parse().ok()).unwrap_or(1);
            let spans = inline(&blocks_of(node, style, depth));
            if spans.iter().any(|s| is_visible(&s.text)) {
                flow.block(Block::Heading { level, spans });
            } else {
                flow.flush();
            }
        }
        "ul" | "ol" | "menu" => {
            let ordered = name == "ol";
            let start = attr(attrs, "start")
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(1);
            let items = list_items(node, style, depth);
            if items.is_empty() {
                flow.flush();
            } else {
                flow.block(Block::List {
                    ordered,
                    start,
                    items,
                });
            }
        }
        "blockquote" => {
            let blocks = blocks_of(node, style, depth);
            if blocks.is_empty() {
                flow.flush();
            } else {
                flow.block(Block::Quote { blocks });
            }
        }
        "pre" | "xmp" | "listing" | "plaintext" => {
            let text = text_content(node);
            let text = text.trim_end();
            if is_visible(text) {
                flow.block(Block::Preformatted {
                    text: text.to_owned(),
                });
            } else {
                flow.flush();
            }
        }
        "hr" => flow.block(Block::Rule),
        "table" => table(node, attrs, flow, style, depth),
        "li" => {
            // An item outside a list.
            flow.flush();
            flow.push("• ".to_owned(), style);
            walk(node, flow, style, depth);
            flow.flush();
        }
        "p" | "div" | "section" | "article" | "header" | "footer" | "main" | "nav" | "aside"
        | "center" | "address" | "figure" | "figcaption" | "details" | "summary" | "dl" | "dt"
        | "dd" | "form" | "fieldset" | "legend" | "body" | "html" | "tr" | "td" | "th"
        | "tbody" | "thead" | "tfoot" | "caption" => {
            flow.flush();
            walk(node, flow, style, depth);
            flow.flush();
        }
        // Inline and unknown elements: their content, in the surrounding style.
        _ => walk(node, flow, style, depth),
    }
}

fn list_items(node: &Handle, style: &Span, depth: usize) -> Vec<Vec<Block>> {
    let mut items: Vec<Vec<Block>> = Vec::new();
    let children = node.children.borrow().clone();
    for child in &children {
        match &child.data {
            Data::Element { name, attrs, .. } if !is_hidden(&attrs.borrow()) => {
                if &*name.local == "li" {
                    let blocks = blocks_of(child, style, depth + 1);
                    if !blocks.is_empty() {
                        items.push(blocks);
                    }
                } else {
                    // A stray element (often a nested list) belongs to the item before it.
                    let mut flow = Flow::default();
                    element(
                        child,
                        &name.local,
                        &attrs.borrow(),
                        &mut flow,
                        style,
                        depth + 1,
                    );
                    let blocks = flow.finish();
                    match items.last_mut() {
                        Some(last) => last.extend(blocks),
                        None if !blocks.is_empty() => items.push(blocks),
                        None => {}
                    }
                }
            }
            Data::Text(text) if is_visible(&text.borrow()) => {
                let mut flow = Flow::default();
                flow.text(&text.borrow(), style);
                items.push(flow.finish());
            }
            _ => {}
        }
    }
    items
}

fn table(node: &Handle, attrs: &[Attribute], flow: &mut Flow, style: &Span, depth: usize) {
    let mut caption = Vec::new();
    let mut rows: Vec<Vec<(bool, Vec<Block>)>> = Vec::new();
    let children = node.children.borrow().clone();
    for child in children.iter().filter(|c| !hidden_node(c)) {
        match element_name(child) {
            Some("thead" | "tbody" | "tfoot") => rows.extend(table_rows(child, style, depth + 1)),
            Some("tr") => rows.push(row_cells(child, style, depth + 1)),
            Some("caption") => caption = blocks_of(child, style, depth + 1),
            _ => {}
        }
    }
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let layout = attr(attrs, "role").is_some_and(|r| r.eq_ignore_ascii_case("presentation"))
        || !(2..=MAX_COLUMNS).contains(&columns)
        || rows.iter().flatten().any(|(_, blocks)| {
            blocks.len() > 1 || blocks.iter().any(|b| !matches!(b, Block::Paragraph { .. }))
        });
    for block in caption {
        flow.block(block);
    }
    if layout {
        for (_, blocks) in rows.into_iter().flatten() {
            for block in blocks {
                flow.block(block);
            }
        }
        flow.flush();
        return;
    }
    let rows: Vec<Vec<Cell>> = rows
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|(header, blocks)| Cell {
                    header,
                    spans: inline(&blocks),
                })
                .collect::<Vec<_>>()
        })
        .filter(|row| row.iter().any(|cell| !cell.spans.is_empty()))
        .collect();
    if rows.is_empty() {
        flow.flush();
    } else {
        flow.block(Block::Table { rows });
    }
}

fn table_rows(group: &Handle, style: &Span, depth: usize) -> Vec<Vec<(bool, Vec<Block>)>> {
    let children = group.children.borrow().clone();
    children
        .iter()
        .filter(|child| element_name(child) == Some("tr") && !hidden_node(child))
        .map(|row| row_cells(row, style, depth))
        .collect()
}

fn row_cells(row: &Handle, style: &Span, depth: usize) -> Vec<(bool, Vec<Block>)> {
    let children = row.children.borrow().clone();
    children
        .iter()
        .filter(|cell| !hidden_node(cell))
        .filter_map(|cell| match element_name(cell) {
            Some("td") => Some((false, blocks_of(cell, style, depth + 1))),
            Some("th") => {
                let mut bold = style.clone();
                bold.bold = true;
                Some((true, blocks_of(cell, &bold, depth + 1)))
            }
            _ => None,
        })
        .collect()
}

/// The text of blocks as one run of spans (paragraphs joined by line breaks).
fn inline(blocks: &[Block]) -> Vec<Span> {
    let mut flow = Flow::default();
    inline_into(blocks, &mut flow);
    flow.spans
}

fn inline_into(blocks: &[Block], flow: &mut Flow) {
    for block in blocks {
        let spans = match block {
            Block::Paragraph { spans } | Block::Heading { spans, .. } => spans.clone(),
            Block::Preformatted { text } => vec![Span {
                text: text.clone(),
                code: true,
                ..Span::default()
            }],
            Block::Table { rows } => rows
                .iter()
                .flatten()
                .flat_map(|cell| cell.spans.clone())
                .collect(),
            Block::List { items, .. } => {
                items.iter().for_each(|item| inline_into(item, flow));
                continue;
            }
            Block::Quote { blocks } | Block::Fold { blocks, .. } => {
                inline_into(blocks, flow);
                continue;
            }
            Block::Rule => continue,
        };
        if !flow.spans.is_empty() {
            flow.push("\n".to_owned(), &Span::default());
        }
        for span in spans {
            let style = span.clone();
            flow.push(span.text, &style);
        }
    }
}

/// All text below `node`, as it is (for `<pre>` and for content nested too deeply). Not
/// recursive.
fn text_content(node: &Handle) -> String {
    let mut out = String::new();
    let mut stack = vec![Rc::clone(node)];
    while let Some(next) = stack.pop() {
        match &next.data {
            Data::Text(text) => out.push_str(&text.borrow()),
            Data::Element { name, attrs, .. } => {
                if &*name.local == "br" {
                    out.push('\n');
                }
                if is_hidden(&attrs.borrow())
                    || matches!(&*name.local, "script" | "style" | "template")
                {
                    continue;
                }
                stack.extend(next.children.borrow().iter().rev().cloned());
            }
            Data::Document => stack.extend(next.children.borrow().iter().rev().cloned()),
            Data::Other => {}
        }
    }
    out
}

fn element_name(node: &Handle) -> Option<&str> {
    match &node.data {
        Data::Element { name, .. } => Some(&*name.local),
        _ => None,
    }
}

fn hidden_node(node: &Handle) -> bool {
    match &node.data {
        Data::Element { attrs, .. } => is_hidden(&attrs.borrow()),
        _ => false,
    }
}

fn attr<'a>(attrs: &'a [Attribute], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|a| &*a.name.local == name)
        .map(|a| a.value.as_ref())
}

/// `hidden`, or an inline style that hides the element (`display: none`, `visibility: hidden`,
/// Outlook's `mso-hide: all`).
fn is_hidden(attrs: &[Attribute]) -> bool {
    if attr(attrs, "hidden").is_some() {
        return true;
    }
    attr(attrs, "style").is_some_and(|style| {
        let style: String = style
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_ascii_lowercase();
        style.split(';').any(|decl| {
            let decl = decl.trim_end_matches("!important");
            matches!(decl, "display:none" | "visibility:hidden" | "mso-hide:all")
        })
    })
}

/// The `Content-ID` an image source names (`cid:…`, without angle brackets).
fn content_id(src: &str) -> Option<String> {
    let src = src.trim();
    let id = src
        .get(..4)
        .filter(|scheme| scheme.eq_ignore_ascii_case("cid:"))
        .and_then(|_| src.get(4..))?;
    let id = id.trim().trim_start_matches('<').trim_end_matches('>');
    (!id.is_empty() && !id.chars().any(char::is_control)).then(|| id.to_owned())
}

/// The link target if it is one dail opens: `http`, `https` or `mailto`, without control
/// characters.
fn safe_link(href: &str) -> Option<String> {
    let href = href.trim();
    let lower = href.get(..8).unwrap_or(href).to_ascii_lowercase();
    let allowed = ["https://", "http://", "mailto:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme) && href.len() > scheme.len());
    (allowed && !href.chars().any(char::is_control)).then(|| href.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(spans: &[Span]) -> String {
        spans.iter().map(|s| s.text.as_str()).collect()
    }

    fn para(block: &Block) -> String {
        match block {
            Block::Paragraph { spans } => text(spans),
            other => format!("{other:?}"),
        }
    }

    #[test]
    fn paragraphs_collapse_white_space_and_keep_line_breaks() {
        let doc = html_document("<p>  Hello \n  <b>big</b>   world </p><p>a<br>  b<br></p>");
        assert_eq!(doc.blocks.len(), 2);
        assert_eq!(para(&doc.blocks[0]), "Hello big world");
        assert_eq!(para(&doc.blocks[1]), "a\nb");
        let Block::Paragraph { spans } = &doc.blocks[0] else {
            unreachable!()
        };
        assert!(spans[1].bold && spans[1].text == "big");
        assert!(!spans[0].bold && !spans[2].bold);
    }

    #[test]
    fn links_keep_only_safe_targets() {
        let doc = html_document(
            r#"<p><a href=" https://example.org/x ">site</a> <a href="javascript:alert(1)">js</a>
            <a href="MAILTO:bob@example.org">mail</a> <a href="data:text/html,x">data</a></p>"#,
        );
        let Block::Paragraph { spans } = &doc.blocks[0] else {
            unreachable!()
        };
        let links: Vec<_> = spans.iter().filter_map(|s| s.link.as_deref()).collect();
        assert_eq!(links, ["https://example.org/x", "MAILTO:bob@example.org"]);
        assert_eq!(text(spans), "site js mail data");
        assert_eq!(
            doc.links(),
            ["https://example.org/x", "MAILTO:bob@example.org"]
        );
    }

    #[test]
    fn scripts_styles_hidden_and_media_are_dropped() {
        let doc = html_document(
            r#"<html><head><title>T</title><style>p{}</style></head><body>
            <div style="display: none !important; max-height:0">preview &zwnj; text</div>
            <script>alert(1)</script><span hidden>secret</span>
            <img src="https://track.example/p.gif" width=1><img src="cid:logo" alt="Logo">
            <iframe src="https://x"></iframe><p>Shown</p></body></html>"#,
        );
        let all: Vec<String> = doc.blocks.iter().map(para).collect();
        assert_eq!(all, ["[Logo]", "Shown"]);
        assert_eq!(
            doc.images(),
            ["logo"],
            "only the message's own image is kept"
        );
    }

    #[test]
    fn headings_lists_quotes_pre_and_rules() {
        let doc = html_document(
            "<h2>Title <i>x</i></h2><ol start=3><li>one<li><p>two</p><ul><li>deep</ul></ol>\
             <blockquote><p>quoted</p></blockquote><pre>  a\n    b\n</pre><hr>",
        );
        let [heading, list, quote, pre, rule] = doc.blocks.as_slice() else {
            unreachable!("{:?}", doc.blocks)
        };
        assert!(matches!(heading, Block::Heading { level: 2, spans } if text(spans) == "Title x"));
        let Block::List {
            ordered: true,
            start: 3,
            items,
        } = list
        else {
            unreachable!("{list:?}")
        };
        assert_eq!(items.len(), 2);
        assert_eq!(para(&items[0][0]), "one");
        assert_eq!(para(&items[1][0]), "two");
        assert!(
            matches!(&items[1][1], Block::List { ordered: false, items, .. } if para(&items[0][0]) == "deep")
        );
        assert!(matches!(quote, Block::Quote { blocks } if para(&blocks[0]) == "quoted"));
        assert_eq!(
            pre,
            &Block::Preformatted {
                text: "  a\n    b".to_owned()
            }
        );
        assert_eq!(rule, &Block::Rule);
    }

    #[test]
    fn data_tables_stay_and_layout_tables_flatten() {
        let doc = html_document(
            "<table><tr><th>Name<th>Qty</tr><tr><td>Apples</td><td><b>3</b></td></tr>\
             <tr><td></td><td></td></tr></table>",
        );
        let [Block::Table { rows }] = doc.blocks.as_slice() else {
            unreachable!("{:?}", doc.blocks)
        };
        assert_eq!(rows.len(), 2);
        assert!(rows[0][0].header && text(&rows[0][0].spans) == "Name");
        assert!(!rows[1][1].header && rows[1][1].spans[0].bold);

        let layout = html_document(
            "<table role=presentation><tr><td>Hi</td><td>there</td></tr></table>\
             <table><tr><td><table><tr><td>a</td><td>b</td></tr></table><p>c</p></td></tr></table>\
             <table><tr><td>only column</td></tr></table>",
        );
        // The outer table of the second one holds a table and a paragraph in one cell: layout.
        // The inner one is a data table.
        let all: Vec<String> = layout.blocks.iter().map(para).collect();
        assert_eq!(all[..2], ["Hi", "there"]);
        assert!(matches!(&layout.blocks[2], Block::Table { rows } if rows[0].len() == 2));
        assert_eq!(all[3..], ["c", "only column"]);
    }

    #[test]
    fn quotes_and_signatures_fold() {
        let gmail = html_document(
            r#"<div dir="ltr">Sounds good.<br clear="all"><div><br></div><span class="gmail_signature_prefix">-- </span><br>
            <div dir="ltr" class="gmail_signature" data-smartmail="gmail_signature">Bob<br>CTO</div></div><br>
            <div class="gmail_quote gmail_quote_container"><div dir="ltr" class="gmail_attr">On Mon, Alice wrote:<br></div>
            <blockquote class="gmail_quote" style="margin:0"><div>Lunch?</div>
            <blockquote type="cite">older</blockquote></blockquote></div>"#,
        );
        let kinds: Vec<_> = gmail
            .blocks
            .iter()
            .map(|b| match b {
                Block::Fold { kind, .. } => format!("{kind:?}"),
                other => para(other),
            })
            .collect();
        assert_eq!(kinds, ["Sounds good.", "Signature", "Quote"]);
        let Block::Fold {
            blocks: signature, ..
        } = &gmail.blocks[1]
        else {
            unreachable!()
        };
        assert_eq!(
            para(&signature[0]),
            "--",
            "the dashes go with the signature"
        );
        let Block::Fold { blocks, .. } = &gmail.blocks[2] else {
            unreachable!()
        };
        assert_eq!(para(&blocks[0]), "On Mon, Alice wrote:");
        let Block::Quote { blocks: quoted } = &blocks[1] else {
            unreachable!("{blocks:?}")
        };
        assert_eq!(para(&quoted[0]), "Lunch?");
        assert!(
            matches!(&quoted[1], Block::Quote { .. }),
            "nested folds become quotes"
        );

        let outlook = html_document(
            r#"<div>Yes.</div><div id="appendonsend"></div><hr><div id="divRplyFwdMsg"><b>From:</b> Alice</div><div>Lunch?</div>"#,
        );
        assert_eq!(para(&outlook.blocks[0]), "Yes.");
        assert!(
            matches!(&outlook.blocks[1], Block::Fold { kind: FoldKind::Quote, blocks } if blocks.len() == 3)
        );

        let dashes = html_document("<p>Thanks</p><p>-- <br>Bob<br>bob.example</p>");
        assert!(matches!(
            &dashes.blocks[1],
            Block::Fold {
                kind: FoldKind::Signature,
                ..
            }
        ));
        let long = format!("<p>Hi</p><p>--<br>{}</p>", "line<br>".repeat(20));
        assert!(
            !html_document(&long)
                .blocks
                .iter()
                .any(|b| matches!(b, Block::Fold { .. }))
        );
    }

    #[test]
    fn deep_nesting_is_reduced_to_text() {
        let html = format!(
            "{}deep{}",
            "<div><span>".repeat(5000),
            "</span></div>".repeat(5000)
        );
        let doc = html_document(&html);
        assert_eq!(doc.blocks.iter().map(para).collect::<Vec<_>>(), ["deep"]);
    }

    #[test]
    fn garbage_and_empty_input() {
        assert!(html_document("").blocks.is_empty());
        let garbage = html_document("<<<>>></p></table>&bogus;");
        let all: Vec<String> = garbage.blocks.iter().map(para).collect();
        assert_eq!(all.concat(), "<<<>>>&bogus;");
        assert!(
            html_document("<p> &nbsp; </p><p>\u{200c}\u{34f}</p>")
                .blocks
                .is_empty()
        );
    }
}
