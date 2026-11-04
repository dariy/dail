//! Plain-text bodies as a [`Document`] for reading (MISC-28): paragraphs reflowed to the reader's width, runs of
//! blank lines collapsed, headings, the safe part of Markdown, and numbered links.
//!
//! The text is untrusted: nothing here loads or runs anything. Markdown images and raw HTML stay text; links are
//! kept only for `http`, `https` and `mailto` targets.
//!
//! - **Paragraphs.** Lines a sender's mailer wrapped (at least [`WRAPPED`] characters long) are joined with the next
//!   line; shorter lines keep their break, so addresses, greetings and signatures stay as written.
//! - **Headings.** Markdown `#` lines, setext underlines (`===`, `---`), a short line in capitals, and a short line
//!   without final punctuation that stands alone before a longer paragraph.
//! - **Markdown.** Lists (`-`, `*`, `+`, `•`, `1.`), block quotes (`>`), fenced and indented code, rules, and inline
//!   `**strong**`, `*emphasis*`, `` `code` ``, `~~strike~~` and `[text](url)`.
//! - **Folds.** The quoted earlier messages and the signature fold away, as `app_core::fold` finds them.

use app_core::document::{Block, Document, FoldKind, Span};

/// A line at least this long was wrapped by the sender's mailer, so the next line continues its paragraph.
pub const WRAPPED: usize = 60;

/// The longest line that can be a guessed heading.
const HEADING_MAX: usize = 60;

/// `text` as a document, its links numbered.
pub fn plain_document(text: &str) -> Document {
    let text = text.replace("\r\n", "\n");
    let folded = app_core::fold::split(&text);
    let mut blocks = blocks(folded.main);
    if let Some(signature) = folded.signature {
        blocks.push(Block::Fold {
            kind: FoldKind::Signature,
            blocks: blocks_of_lines(&signature.lines().collect::<Vec<_>>(), false),
        });
    }
    if let Some(quote) = folded.quote {
        blocks.push(Block::Fold {
            kind: FoldKind::Quote,
            blocks: blocks_of_lines(&quote.lines().collect::<Vec<_>>(), false),
        });
    }
    let mut document = Document { blocks };
    document.number_links();
    document
}

fn blocks(text: &str) -> Vec<Block> {
    blocks_of_lines(&text.lines().collect::<Vec<_>>(), true)
}

/// The blocks of `lines`. `guess` turns on the guessed headings (not inside quotes and folds).
fn blocks_of_lines(lines: &[&str], guess: bool) -> Vec<Block> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim_end();
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            i += 1;
            continue;
        }
        // Fenced code.
        if let Some(fence) = ["```", "~~~"].into_iter().find(|f| trimmed.starts_with(f)) {
            let mut text = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with(fence) {
                text.push(lines[i]);
                i += 1;
            }
            i += 1;
            out.push(Block::Preformatted {
                text: text.join("\n"),
            });
            continue;
        }
        // Indented code: four spaces or a tab, after a blank line or at the start.
        if (line.starts_with("    ") || line.starts_with('\t'))
            && (i == 0 || lines[i - 1].trim().is_empty())
            && !is_item(trimmed)
        {
            let mut text = Vec::new();
            while i < lines.len()
                && (lines[i].starts_with("    ")
                    || lines[i].starts_with('\t')
                    || lines[i].trim().is_empty())
            {
                text.push(
                    lines[i]
                        .strip_prefix("    ")
                        .or_else(|| lines[i].strip_prefix('\t'))
                        .unwrap_or(""),
                );
                i += 1;
            }
            while text.last().is_some_and(|l| l.trim().is_empty()) {
                text.pop();
            }
            out.push(Block::Preformatted {
                text: text.join("\n"),
            });
            continue;
        }
        // Block quote.
        if trimmed.starts_with('>') {
            let mut inner = Vec::new();
            while i < lines.len() && lines[i].trim_start().starts_with('>') {
                let l = lines[i].trim_start();
                let l = l.strip_prefix('>').unwrap_or(l);
                inner.push(l.strip_prefix(' ').unwrap_or(l));
                i += 1;
            }
            out.push(Block::Quote {
                blocks: blocks_of_lines(&inner, false),
            });
            continue;
        }
        // ATX heading.
        if let Some((level, title)) = atx_heading(trimmed) {
            out.push(Block::Heading {
                level,
                spans: inline(title),
            });
            i += 1;
            continue;
        }
        // Rule.
        if is_rule(trimmed) {
            out.push(Block::Rule);
            i += 1;
            continue;
        }
        // List.
        if is_item(trimmed) {
            let ordered = ordered_number(trimmed).is_some();
            let start = ordered_number(trimmed).unwrap_or(1);
            let mut items: Vec<Vec<Block>> = Vec::new();
            while i < lines.len() {
                let l = lines[i].trim_end();
                let t = l.trim_start();
                if t.is_empty() {
                    // A blank line ends the list unless an item follows.
                    if lines.get(i + 1).is_some_and(|n| is_item(n.trim_start())) {
                        i += 1;
                        continue;
                    }
                    break;
                }
                if is_item(t) && (ordered_number(t).is_some() == ordered) {
                    let body = item_text(t);
                    items.push(vec![Block::Paragraph {
                        spans: inline(body),
                    }]);
                } else if l.starts_with(' ') || l.starts_with('\t') {
                    // A continuation of the item.
                    if let Some(Block::Paragraph { spans }) =
                        items.last_mut().and_then(|item| item.last_mut())
                    {
                        spans.push(Span {
                            text: " ".to_owned(),
                            ..Span::default()
                        });
                        spans.extend(inline(t));
                    }
                } else {
                    break;
                }
                i += 1;
            }
            out.push(Block::List {
                ordered,
                start,
                items,
            });
            continue;
        }
        // A paragraph: up to a blank line or another block. A setext underline makes it a heading.
        let mut para: Vec<&str> = Vec::new();
        while i < lines.len() {
            let l = lines[i].trim_end();
            let t = l.trim_start();
            if t.is_empty()
                || t.starts_with('>')
                || t.starts_with("```")
                || t.starts_with("~~~")
                || atx_heading(t).is_some()
                || (is_item(t)
                    && !para.is_empty()
                    && para.last().is_some_and(|p| p.len() < WRAPPED))
            {
                break;
            }
            if !para.is_empty() && is_setext(t) {
                break;
            }
            if para.is_empty() && is_rule(t) {
                break;
            }
            para.push(t);
            i += 1;
        }
        if let Some(underline) = lines
            .get(i)
            .map(|l| l.trim())
            .filter(|t| !para.is_empty() && is_setext(t))
        {
            let level = if underline.starts_with('=') { 1 } else { 2 };
            out.push(Block::Heading {
                level,
                spans: inline(&para.join(" ")),
            });
            i += 1;
            continue;
        }
        if para.is_empty() {
            i += 1;
            continue;
        }
        let blank_before = i - para.len() == 0 || lines[i - para.len() - 1].trim().is_empty();
        let blank_after = lines.get(i).is_none_or(|l| l.trim().is_empty());
        if guess
            && para.len() == 1
            && blank_before
            && blank_after
            && guessed_heading(para[0], lines.get(i + 1..).unwrap_or_default())
        {
            out.push(Block::Heading {
                level: 3,
                spans: inline(para[0]),
            });
            continue;
        }
        out.push(Block::Paragraph {
            spans: inline(&reflow(&para)),
        });
    }
    out
}

/// The lines of a paragraph joined where the sender's mailer wrapped them; a short line keeps its break.
fn reflow(lines: &[&str]) -> String {
    let mut out = String::new();
    for (n, line) in lines.iter().enumerate() {
        if n > 0 {
            let previous = lines[n - 1];
            let hard = previous.chars().count() < WRAPPED || previous.ends_with('\\');
            out.push(if hard { '\n' } else { ' ' });
        }
        out.push_str(line.strip_suffix('\\').unwrap_or(line));
    }
    out
}

fn atx_heading(line: &str) -> Option<(u8, &str)> {
    let hashes = line.chars().take_while(|&c| c == '#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = line.get(hashes..)?;
    let title = rest
        .strip_prefix(' ')?
        .trim()
        .trim_end_matches('#')
        .trim_end();
    (!title.is_empty()).then(|| (u8::try_from(hashes).unwrap_or(6), title))
}

fn is_setext(line: &str) -> bool {
    line.len() >= 3 && (line.chars().all(|c| c == '=') || line.chars().all(|c| c == '-'))
}

fn is_rule(line: &str) -> bool {
    let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    compact.len() >= 3
        && ['-', '*', '_']
            .iter()
            .any(|&m| compact.chars().all(|c| c == m))
}

fn is_item(line: &str) -> bool {
    ["- ", "* ", "+ ", "• "].iter().any(|m| line.starts_with(m)) || ordered_number(line).is_some()
}

fn ordered_number(line: &str) -> Option<u32> {
    let digits: String = line.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() || digits.len() > 4 {
        return None;
    }
    let rest = line.get(digits.len()..)?;
    (rest.starts_with(". ") || rest.starts_with(") "))
        .then(|| digits.parse().ok())
        .flatten()
}

fn item_text(line: &str) -> &str {
    if let Some(n) = ordered_number(line) {
        let skip = n.to_string().len() + 2;
        return line.get(skip..).unwrap_or("").trim_start();
    }
    line.get(2..).unwrap_or("").trim_start()
}

/// A line alone between blank lines that reads as a title: in capitals, or short, starting with a capital and
/// without final punctuation, before a longer paragraph (not a greeting or a name at the end).
fn guessed_heading(line: &str, after: &[&str]) -> bool {
    let chars = line.chars().count();
    if !(3..=HEADING_MAX).contains(&chars) {
        return false;
    }
    let letters: Vec<char> = line.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() >= 3 && letters.iter().all(|c| c.is_uppercase()) {
        return true;
    }
    let ends_open = !line.ends_with(['.', ',', ':', ';', '!', '?', ')', '"', '\'']);
    let capital = line.chars().next().is_some_and(char::is_uppercase);
    let words = line.split_whitespace().count();
    // What follows: the next paragraph must be a real one (two lines, or one long one).
    let next: Vec<&str> = after
        .iter()
        .map(|l| l.trim())
        .take_while(|l| !l.is_empty())
        .collect();
    let long_next = next.len() >= 2 || next.first().is_some_and(|l| l.chars().count() >= WRAPPED);
    ends_open
        && capital
        && (2..=8).contains(&words)
        && long_next
        && !line.contains("://")
        && !line.contains('@')
}

/// Inline Markdown: `**strong**`, `__strong__`, `*em*`, `_em_`, `` `code` ``, `~~strike~~`, `[text](url)`, `<url>`
/// and bare `http(s)://` and `mailto:` URLs. What does not close is text.
pub fn inline(text: &str) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    let mut style = Span::default();
    let mut plain = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let flush = |plain: &mut String, out: &mut Vec<Span>, style: &Span| {
        if !plain.is_empty() {
            push(
                out,
                Span {
                    text: std::mem::take(plain),
                    ..style.clone()
                },
            );
        }
    };
    while i < chars.len() {
        let rest: String = chars[i..].iter().collect();
        // Code: everything to the closing backtick is literal.
        if chars[i] == '`'
            && let Some(end) = rest[1..].find('`')
        {
            flush(&mut plain, &mut out, &style);
            let code = &rest[1..1 + end];
            push(
                &mut out,
                Span {
                    text: code.to_owned(),
                    code: true,
                    ..style.clone()
                },
            );
            i += 2 + code.chars().count();
            continue;
        }
        // A Markdown link.
        if chars[i] == '['
            && let Some((label, url, used)) = markdown_link(&rest)
        {
            flush(&mut plain, &mut out, &style);
            for mut span in inline(label) {
                span.link = Some(url.clone());
                push(
                    &mut out,
                    Span {
                        bold: span.bold || style.bold,
                        italic: span.italic || style.italic,
                        ..span
                    },
                );
            }
            i += used;
            continue;
        }
        // An autolink `<https://…>`.
        if chars[i] == '<'
            && let Some(end) = rest.find('>')
            && let Some(url) = safe_url(&rest[1..end])
        {
            flush(&mut plain, &mut out, &style);
            push(
                &mut out,
                Span {
                    text: url.clone(),
                    link: Some(url),
                    ..style.clone()
                },
            );
            i += rest[..=end].chars().count();
            continue;
        }
        // A bare URL.
        if (i == 0 || !chars[i - 1].is_alphanumeric())
            && let Some((url, used)) = bare_url(&rest)
        {
            flush(&mut plain, &mut out, &style);
            push(
                &mut out,
                Span {
                    text: url.clone(),
                    link: Some(url),
                    ..style.clone()
                },
            );
            i += used;
            continue;
        }
        // Emphasis markers toggle when they can open or close.
        let pair = |m: &str| rest.starts_with(m);
        let opens = |len: usize| chars.get(i + len).is_some_and(|c| !c.is_whitespace());
        let closes = || i > 0 && !chars[i - 1].is_whitespace();
        let word_inside = |len: usize| {
            // `_` inside a word (snake_case) is not emphasis.
            i > 0
                && chars[i - 1].is_alphanumeric()
                && chars.get(i + len).is_some_and(|c| c.is_alphanumeric())
        };
        let toggle = |flag: bool, len: usize| {
            (flag && closes())
                || (!flag && opens(len) && has_closer(&chars, i + len, &chars[i..i + len]))
        };
        if (pair("**") || pair("__")) && toggle(style.bold, 2) && !(pair("__") && word_inside(2)) {
            flush(&mut plain, &mut out, &style);
            style.bold = !style.bold;
            i += 2;
            continue;
        }
        if pair("~~") && toggle(style.strike, 2) {
            flush(&mut plain, &mut out, &style);
            style.strike = !style.strike;
            i += 2;
            continue;
        }
        if (chars[i] == '*' || chars[i] == '_')
            && toggle(style.italic, 1)
            && !(chars[i] == '_' && word_inside(1))
        {
            flush(&mut plain, &mut out, &style);
            style.italic = !style.italic;
            i += 1;
            continue;
        }
        plain.push(chars[i]);
        i += 1;
    }
    flush(&mut plain, &mut out, &style);
    out
}

/// Whether `marker` occurs again after `from` (so an opening marker has a closer).
fn has_closer(chars: &[char], from: usize, marker: &[char]) -> bool {
    (from..chars.len().saturating_sub(marker.len() - 1))
        .any(|j| chars[j..j + marker.len()] == *marker && j > from && !chars[j - 1].is_whitespace())
}

/// Appends `span`, merged into the previous one if they have the same style.
fn push(out: &mut Vec<Span>, span: Span) {
    if span.text.is_empty() {
        return;
    }
    match out.last_mut() {
        Some(last) if last.same_style(&span) => last.text.push_str(&span.text),
        _ => out.push(span),
    }
}

/// `[label](url)` at the start of `text`: the label, the URL and the characters used. Only safe targets.
fn markdown_link(text: &str) -> Option<(&str, String, usize)> {
    let close = text.find("](")?;
    let label = &text[1..close];
    if label.is_empty() || label.contains('[') || label.contains('\n') {
        return None;
    }
    let after = &text[close + 2..];
    let end = after.find(')')?;
    let target = after[..end].trim();
    let url = safe_url(target.split_whitespace().next()?)?;
    let used = text[..close + 2 + end + 1].chars().count();
    Some((label, url, used))
}

/// `url` if it is an `http`, `https` or `mailto` URL without spaces.
fn safe_url(url: &str) -> Option<String> {
    let lower = url.to_ascii_lowercase();
    let ok = (lower.starts_with("https://")
        || lower.starts_with("http://")
        || lower.starts_with("mailto:"))
        && url.len() > 8
        && !url.chars().any(char::is_whitespace);
    ok.then(|| url.to_owned())
}

/// A bare URL at the start of `text`, without trailing punctuation: the URL and the characters used.
fn bare_url(text: &str) -> Option<(String, usize)> {
    let lower = text.get(..8)?.to_ascii_lowercase();
    if !(lower.starts_with("https://")
        || lower.starts_with("http://")
        || lower.starts_with("mailto:"))
    {
        return None;
    }
    let end = text
        .find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '`'))
        .unwrap_or(text.len());
    let mut url = &text[..end];
    // A closing bracket belongs to the URL only if it opened inside it (Wikipedia's `(film)`).
    loop {
        let trimmed = url.trim_end_matches(['.', ',', ';', ':', '!', '?', '\'']);
        let trimmed = if trimmed.ends_with(')')
            && trimmed.matches('(').count() < trimmed.matches(')').count()
        {
            &trimmed[..trimmed.len() - 1]
        } else {
            trimmed
        };
        if trimmed.len() == url.len() {
            break;
        }
        url = trimmed;
    }
    let url = safe_url(url)?;
    let used = url.chars().count();
    Some((url, used))
}
