//! Text formatting parsers: HTML and Markdown ↔ Telegram [`MessageEntity`]
//!
//! # Markdown (Telegram-flavoured)
//! Supported: `**bold**`, `__italic__`, `~~strike~~`, `||spoiler||`, `` `code` ``,
//! ` ```lang\npre``` `, `[text](url)`, `[text](tg://user?id=123)`
//!
//! # HTML
//! Supported tags: `<b>`, `<strong>`, `<i>`, `<em>`, `<u>`, `<s>`, `<del>`,
//! `<code>`, `<pre>`, `<tg-spoiler>`, `<a href="url">`,
//! `<tg-emoji emoji-id="id">text</tg-emoji>`
//!
//! # Feature gates
//! * `html`      — enables `parse_html` / `generate_html` via the built-in hand-rolled
//!   parser (zero extra deps).
//! * `html5ever` — replaces `parse_html` with a spec-compliant html5ever tokenizer.
//!   `generate_html` is always the same hand-rolled generator.

use layer_tl_types as tl;

// ─── Markdown ─────────────────────────────────────────────────────────────────

/// Parse Telegram-flavoured markdown into (plain_text, entities).
pub fn parse_markdown(text: &str) -> (String, Vec<tl::enums::MessageEntity>) {
    let mut out   = String::with_capacity(text.len());
    let mut ents  = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut i = 0;
    let mut open_stack: Vec<(MarkdownTag, i32)> = Vec::new();
    let mut utf16_off: i32 = 0;

    macro_rules! push_char {
        ($c:expr) => {{ let c: char = $c; out.push(c); utf16_off += c.len_utf16() as i32; }};
    }

    while i < n {
        // ── code block ```lang\n...``` ──────────────────────────────────────
        if i + 2 < n && chars[i] == '`' && chars[i+1] == '`' && chars[i+2] == '`' {
            let start = i + 3;
            let mut j = start;
            while j + 2 < n {
                if chars[j] == '`' && chars[j+1] == '`' && chars[j+2] == '`' { break; }
                j += 1;
            }
            if j + 2 < n {
                let block: String = chars[start..j].iter().collect();
                let (lang, code) = if let Some(nl) = block.find('\n') {
                    (block[..nl].trim().to_string(), block[nl+1..].to_string())
                } else { (String::new(), block) };
                let code_off = utf16_off;
                let code_utf16: i32 = code.encode_utf16().count() as i32;
                ents.push(tl::enums::MessageEntity::Pre(tl::types::MessageEntityPre {
                    offset: code_off, length: code_utf16, language: lang,
                }));
                for c in code.chars() { push_char!(c); }
                i = j + 3;
                continue;
            }
        }

        // ── inline code ─────────────────────────────────────────────────────
        if chars[i] == '`' {
            let start = i + 1;
            let mut j = start;
            while j < n && chars[j] != '`' { j += 1; }
            if j < n {
                let code: String = chars[start..j].iter().collect();
                let code_off = utf16_off;
                let code_utf16: i32 = code.encode_utf16().count() as i32;
                ents.push(tl::enums::MessageEntity::Code(tl::types::MessageEntityCode {
                    offset: code_off, length: code_utf16,
                }));
                for c in code.chars() { push_char!(c); }
                i = j + 1;
                continue;
            }
        }

        // ── [text](url) ─────────────────────────────────────────────────────
        if chars[i] == '[' {
            let text_start = i + 1;
            let mut j = text_start;
            let mut depth = 1i32;
            while j < n {
                if chars[j] == '[' { depth += 1; }
                if chars[j] == ']' { depth -= 1; if depth == 0 { break; } }
                j += 1;
            }
            if j < n && j + 1 < n && chars[j+1] == '(' {
                let link_start = j + 2;
                let mut k = link_start;
                while k < n && chars[k] != ')' { k += 1; }
                if k < n {
                    let inner_text: String = chars[text_start..j].iter().collect();
                    let url: String = chars[link_start..k].iter().collect();
                    const MENTION_PFX: &str = "tg://user?id=";
                    let ent_off = utf16_off;
                    for c in inner_text.chars() { push_char!(c); }
                    let ent_len = utf16_off - ent_off;
                    if let Some(stripped) = url.strip_prefix(MENTION_PFX) {
                        if let Ok(uid) = stripped.parse::<i64>() {
                            ents.push(tl::enums::MessageEntity::MentionName(
                                tl::types::MessageEntityMentionName { offset: ent_off, length: ent_len, user_id: uid }
                            ));
                        }
                    } else {
                        ents.push(tl::enums::MessageEntity::TextUrl(
                            tl::types::MessageEntityTextUrl { offset: ent_off, length: ent_len, url }
                        ));
                    }
                    i = k + 1;
                    continue;
                }
            }
        }

        // ── two-char delimiters ──────────────────────────────────────────────
        let two: Option<(&str, MarkdownTag)> = if i + 1 < n {
            match [chars[i], chars[i+1]] {
                ['*','*'] => Some(("**", MarkdownTag::Bold)),
                ['_','_'] => Some(("__", MarkdownTag::Italic)),
                ['~','~'] => Some(("~~", MarkdownTag::Strike)),
                ['|','|'] => Some(("||", MarkdownTag::Spoiler)),
                _         => None,
            }
        } else { None };

        if let Some((_delim, tag)) = two {
            if let Some(pos) = open_stack.iter().rposition(|(t, _)| *t == tag) {
                let (_, start_off) = open_stack.remove(pos);
                let length = utf16_off - start_off;
                let entity = match tag {
                    MarkdownTag::Bold    => tl::enums::MessageEntity::Bold(tl::types::MessageEntityBold { offset: start_off, length }),
                    MarkdownTag::Italic  => tl::enums::MessageEntity::Italic(tl::types::MessageEntityItalic { offset: start_off, length }),
                    MarkdownTag::Strike  => tl::enums::MessageEntity::Strike(tl::types::MessageEntityStrike { offset: start_off, length }),
                    MarkdownTag::Spoiler => tl::enums::MessageEntity::Spoiler(tl::types::MessageEntitySpoiler { offset: start_off, length }),
                };
                if length > 0 { ents.push(entity); }
            } else {
                open_stack.push((tag, utf16_off));
            }
            i += 2;
            continue;
        }

        push_char!(chars[i]);
        i += 1;
    }

    (out, ents)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarkdownTag { Bold, Italic, Strike, Spoiler }

/// Generate Telegram markdown from plain text + entities.
pub fn generate_markdown(text: &str, entities: &[tl::enums::MessageEntity]) -> String {
    use tl::enums::MessageEntity as ME;
    let mut insertions: Vec<(i32, &'static str)> = Vec::new();
    for ent in entities {
        match ent {
            ME::Bold(e)    => { insertions.push((e.offset, "**")); insertions.push((e.offset+e.length, "**")); }
            ME::Italic(e)  => { insertions.push((e.offset, "__")); insertions.push((e.offset+e.length, "__")); }
            ME::Strike(e)  => { insertions.push((e.offset, "~~")); insertions.push((e.offset+e.length, "~~")); }
            ME::Spoiler(e) => { insertions.push((e.offset, "||")); insertions.push((e.offset+e.length, "||")); }
            ME::Code(e)    => { insertions.push((e.offset, "`"));  insertions.push((e.offset+e.length, "`")); }
            _ => {}
        }
    }
    insertions.sort_by_key(|&(pos, _)| pos);

    let mut result = String::with_capacity(text.len() + insertions.len() * 4);
    let mut ins_idx = 0;
    let mut utf16_pos: i32 = 0;
    for ch in text.chars() {
        while ins_idx < insertions.len() && insertions[ins_idx].0 <= utf16_pos {
            result.push_str(insertions[ins_idx].1);
            ins_idx += 1;
        }
        result.push(ch);
        utf16_pos += ch.len_utf16() as i32;
    }
    while ins_idx < insertions.len() { result.push_str(insertions[ins_idx].1); ins_idx += 1; }
    result
}

// ─── HTML parser — built-in hand-rolled (no extra deps) ──────────────────────
// Compiled when `html5ever` feature is NOT active.

/// Parse a Telegram-compatible HTML string into (plain_text, entities).
///
/// Hand-rolled, zero-dependency implementation.  Override with the
/// `html5ever` Cargo feature for a spec-compliant tokenizer.
#[cfg(not(feature = "html5ever"))]
pub fn parse_html(html: &str) -> (String, Vec<tl::enums::MessageEntity>) {
    let mut out        = String::with_capacity(html.len());
    let mut ents       = Vec::new();
    let mut stack: Vec<(HtmlTag, i32, Option<String>)> = Vec::new();
    let mut utf16_off: i32 = 0;

    let bytes = html.as_bytes();
    let len   = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'<' {
            let tag_start = i + 1;
            let mut j = tag_start;
            while j < len && bytes[j] != b'>' { j += 1; }
            let tag_content = &html[tag_start..j];
            i = j + 1;

            let is_close = tag_content.starts_with('/');
            let tag_str  = if is_close { tag_content[1..].trim() } else { tag_content.trim() };
            let (tag_name, attrs) = parse_tag(tag_str);

            if is_close {
                if let Some(pos) = stack.iter().rposition(|(t, _, _)| t.name() == tag_name) {
                    let (htag, start_off, extra) = stack.remove(pos);
                    let length = utf16_off - start_off;
                    if length > 0 {
                        let entity = match htag {
                            HtmlTag::Bold    => Some(tl::enums::MessageEntity::Bold(tl::types::MessageEntityBold { offset: start_off, length })),
                            HtmlTag::Italic  => Some(tl::enums::MessageEntity::Italic(tl::types::MessageEntityItalic { offset: start_off, length })),
                            HtmlTag::Underline => Some(tl::enums::MessageEntity::Underline(tl::types::MessageEntityUnderline { offset: start_off, length })),
                            HtmlTag::Strike  => Some(tl::enums::MessageEntity::Strike(tl::types::MessageEntityStrike { offset: start_off, length })),
                            HtmlTag::Spoiler => Some(tl::enums::MessageEntity::Spoiler(tl::types::MessageEntitySpoiler { offset: start_off, length })),
                            HtmlTag::Code    => Some(tl::enums::MessageEntity::Code(tl::types::MessageEntityCode { offset: start_off, length })),
                            HtmlTag::Pre     => Some(tl::enums::MessageEntity::Pre(tl::types::MessageEntityPre { offset: start_off, length, language: extra.unwrap_or_default() })),
                            HtmlTag::Link(url) => {
                                const PFX: &str = "tg://user?id=";
                                if let Some(stripped) = url.strip_prefix(PFX) {
                                    stripped.parse::<i64>().ok().map(|uid|
                                        tl::enums::MessageEntity::MentionName(tl::types::MessageEntityMentionName { offset: start_off, length, user_id: uid }))
                                } else {
                                    Some(tl::enums::MessageEntity::TextUrl(tl::types::MessageEntityTextUrl { offset: start_off, length, url }))
                                }
                            }
                            HtmlTag::CustomEmoji(id) => Some(tl::enums::MessageEntity::CustomEmoji(tl::types::MessageEntityCustomEmoji { offset: start_off, length, document_id: id })),
                            HtmlTag::Unknown => None,
                        };
                        if let Some(e) = entity { ents.push(e); }
                    }
                }
            } else {
                let htag = match tag_name {
                    "b" | "strong"         => HtmlTag::Bold,
                    "i" | "em"             => HtmlTag::Italic,
                    "u"                    => HtmlTag::Underline,
                    "s" | "del" | "strike" => HtmlTag::Strike,
                    "tg-spoiler"           => HtmlTag::Spoiler,
                    "code"                 => HtmlTag::Code,
                    "pre"                  => HtmlTag::Pre,
                    "a" => HtmlTag::Link(attrs.iter().find(|(k, _)| k == "href").map(|(_, v)| v.clone()).unwrap_or_default()),
                    "tg-emoji" => HtmlTag::CustomEmoji(attrs.iter().find(|(k, _)| k == "emoji-id").and_then(|(_, v)| v.parse::<i64>().ok()).unwrap_or(0)),
                    "br" => { out.push('\n'); utf16_off += 1; continue; }
                    _ => HtmlTag::Unknown,
                };
                stack.push((htag, utf16_off, None));
            }
        } else {
            let text_start = i;
            while i < len && bytes[i] != b'<' { i += 1; }
            let decoded = decode_html_entities(&html[text_start..i]);
            for ch in decoded.chars() { out.push(ch); utf16_off += ch.len_utf16() as i32; }
        }
    }

    (out, ents)
}

#[cfg(not(feature = "html5ever"))]
fn decode_html_entities(s: &str) -> String {
    s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
     .replace("&quot;", "\"").replace("&#39;", "'").replace("&nbsp;", "\u{00A0}")
}

#[cfg(not(feature = "html5ever"))]
fn parse_tag(s: &str) -> (&str, Vec<(String, String)>) {
    let mut parts = s.splitn(2, char::is_whitespace);
    let name  = parts.next().unwrap_or("").trim_end_matches('/');
    let attrs = parse_attrs(parts.next().unwrap_or(""));
    (name, attrs)
}

#[cfg(not(feature = "html5ever"))]
fn parse_attrs(s: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let mut rem = s.trim();
    while !rem.is_empty() {
        if let Some(eq) = rem.find('=') {
            let key = rem[..eq].trim().to_string();
            rem = rem[eq+1..].trim_start();
            let (val, rest) = if let Some(s) = rem.strip_prefix('"') {
                let end = s.find('"').map(|p| p + 1).unwrap_or(rem.len() - 1);
                (rem[1..end].to_string(), &rem[end+1..])
            } else if let Some(s) = rem.strip_prefix('\'') {
                let end = s.find('\'').map(|p| p + 1).unwrap_or(rem.len() - 1);
                (rem[1..end].to_string(), &rem[end+1..])
            } else {
                let end = rem.find(char::is_whitespace).unwrap_or(rem.len());
                (rem[..end].to_string(), &rem[end..])
            };
            result.push((key, val));
            rem = rest.trim_start();
        } else { break; }
    }
    result
}

#[cfg(not(feature = "html5ever"))]
#[allow(dead_code)]
#[derive(Debug, Clone)]
enum HtmlTag {
    Bold, Italic, Underline, Strike, Spoiler, Code, Pre,
    Link(String), CustomEmoji(i64), Unknown,
}

#[cfg(not(feature = "html5ever"))]
impl HtmlTag {
    fn name(&self) -> &str {
        match self {
            Self::Bold           => "b",
            Self::Italic         => "i",
            Self::Underline      => "u",
            Self::Strike         => "s",
            Self::Spoiler        => "tg-spoiler",
            Self::Code           => "code",
            Self::Pre            => "pre",
            Self::Link(_)        => "a",
            Self::CustomEmoji(_) => "tg-emoji",
            Self::Unknown        => "",
        }
    }
}

// ─── HTML parser — html5ever backend ─────────────────────────────────────────
// Compiled when `html5ever` feature IS active; overrides the built-in parser.

/// Parse a Telegram-compatible HTML string into (plain_text, entities).
///
/// Uses the [`html5ever`] spec-compliant tokenizer.  Enable the `html5ever`
/// Cargo feature to activate this implementation.
#[cfg(feature = "html5ever")]
pub fn parse_html(html: &str) -> (String, Vec<tl::enums::MessageEntity>) {
    use std::cell::Cell;
    use html5ever::tendril::StrTendril;
    use html5ever::tokenizer::{
        BufferQueue, Tag, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer,
    };

    struct Sink {
        text:     Cell<String>,
        entities: Cell<Vec<tl::enums::MessageEntity>>,
        offset:   Cell<i32>,
    }

    impl TokenSink for Sink {
        type Handle = ();

        fn process_token(&self, token: Token, _line: u64) -> TokenSinkResult<()> {
            let mut text     = self.text.take();
            let mut entities = self.entities.take();
            let mut offset   = self.offset.get();

            // Close the most-recent open entity of `$kind` (open = length==0).
            // Removes the entity if start == end (zero-length element).
            macro_rules! close_ent {
                ($kind:ident) => {{
                    if let Some(idx) = entities.iter().rposition(|e|
                        matches!(e, tl::enums::MessageEntity::$kind(_)))
                    {
                        let closed_len = {
                            if let tl::enums::MessageEntity::$kind(ref mut inner) = entities[idx] {
                                inner.length = offset - inner.offset;
                                inner.length
                            } else { unreachable!() }
                        };
                        if closed_len == 0 { entities.remove(idx); }
                    }
                }};
            }

            match token {
                // ── Start tags ───────────────────────────────────────────────
                Token::TagToken(Tag { kind: TagKind::StartTag, name, attrs, .. }) => {
                    let len0 = 0i32;
                    match name.as_ref() {
                        "b" | "strong" =>
                            entities.push(tl::enums::MessageEntity::Bold(
                                tl::types::MessageEntityBold { offset, length: len0 })),
                        "i" | "em" =>
                            entities.push(tl::enums::MessageEntity::Italic(
                                tl::types::MessageEntityItalic { offset, length: len0 })),
                        "u" =>
                            entities.push(tl::enums::MessageEntity::Underline(
                                tl::types::MessageEntityUnderline { offset, length: len0 })),
                        "s" | "del" | "strike" =>
                            entities.push(tl::enums::MessageEntity::Strike(
                                tl::types::MessageEntityStrike { offset, length: len0 })),
                        "tg-spoiler" =>
                            entities.push(tl::enums::MessageEntity::Spoiler(
                                tl::types::MessageEntitySpoiler { offset, length: len0 })),
                        "code" => {
                            // Inside an open <pre>? Annotate language on the pre entity.
                            let in_pre = entities.last().map_or(false, |e| {
                                matches!(e, tl::enums::MessageEntity::Pre(p) if p.length == 0)
                            });
                            if in_pre {
                                let lang = attrs.iter()
                                    .find(|a| a.name.local.as_ref() == "class")
                                    .and_then(|a| {
                                        let v: &str = a.value.as_ref();
                                        v.strip_prefix("language-")
                                    })
                                    .map(|s| s.to_string())
                                    .unwrap_or_default();
                                if let Some(tl::enums::MessageEntity::Pre(ref mut p)) = entities.last_mut() {
                                    p.language = lang;
                                }
                            } else {
                                entities.push(tl::enums::MessageEntity::Code(
                                    tl::types::MessageEntityCode { offset, length: len0 }));
                            }
                        }
                        "pre" =>
                            entities.push(tl::enums::MessageEntity::Pre(
                                tl::types::MessageEntityPre { offset, length: len0, language: String::new() })),
                        "a" => {
                            let href = attrs.iter()
                                .find(|a| a.name.local.as_ref() == "href")
                                .map(|a| { let v: &str = a.value.as_ref(); v.to_string() })
                                .unwrap_or_default();
                            const MENTION_PFX: &str = "tg://user?id=";
                            if href.starts_with(MENTION_PFX) {
                                if let Ok(uid) = href[MENTION_PFX.len()..].parse::<i64>() {
                                    entities.push(tl::enums::MessageEntity::MentionName(
                                        tl::types::MessageEntityMentionName { offset, length: len0, user_id: uid }));
                                }
                            } else {
                                entities.push(tl::enums::MessageEntity::TextUrl(
                                    tl::types::MessageEntityTextUrl { offset, length: len0, url: href }));
                            }
                        }
                        "tg-emoji" => {
                            let doc_id = attrs.iter()
                                .find(|a| a.name.local.as_ref() == "emoji-id")
                                .and_then(|a| { let v: &str = a.value.as_ref(); v.parse::<i64>().ok() })
                                .unwrap_or(0);
                            entities.push(tl::enums::MessageEntity::CustomEmoji(
                                tl::types::MessageEntityCustomEmoji { offset, length: len0, document_id: doc_id }));
                        }
                        "br" => { text.push('\n'); offset += 1; }
                        _ => {}
                    }
                }

                // ── End tags ─────────────────────────────────────────────────
                Token::TagToken(Tag { kind: TagKind::EndTag, name, .. }) => {
                    match name.as_ref() {
                        "b" | "strong"         => close_ent!(Bold),
                        "i" | "em"             => close_ent!(Italic),
                        "u"                    => close_ent!(Underline),
                        "s" | "del" | "strike" => close_ent!(Strike),
                        "tg-spoiler"           => close_ent!(Spoiler),
                        "code" => {
                            // Inside open <pre>: pre absorbs the code tag.
                            let in_pre = entities.last().map_or(false, |e| {
                                matches!(e, tl::enums::MessageEntity::Pre(p) if p.length == 0)
                            });
                            if !in_pre { close_ent!(Code); }
                        }
                        "pre"      => close_ent!(Pre),
                        "a" => {
                            match entities.last() {
                                Some(tl::enums::MessageEntity::MentionName(_)) => close_ent!(MentionName),
                                _ => close_ent!(TextUrl),
                            }
                        }
                        "tg-emoji" => close_ent!(CustomEmoji),
                        _ => {}
                    }
                }

                // ── Text content ─────────────────────────────────────────────
                Token::CharacterTokens(s) => {
                    let s_str: &str = s.as_ref();
                    offset += s_str.encode_utf16().count() as i32;
                    text.push_str(s_str);
                }

                _ => {}
            }

            self.text.replace(text);
            self.entities.replace(entities);
            self.offset.replace(offset);
            TokenSinkResult::Continue
        }
    }

    let mut input = BufferQueue::default();
    input.push_back(StrTendril::from_slice(html).try_reinterpret().unwrap());

    let tok = Tokenizer::new(
        Sink {
            text:     Cell::new(String::with_capacity(html.len())),
            entities: Cell::new(Vec::new()),
            offset:   Cell::new(0),
        },
        Default::default(),
    );
    let _ = tok.feed(&mut input);
    tok.end();

    let Sink { text, entities, .. } = tok.sink;
    (text.take(), entities.take())
}

// ─── HTML generator (always available, no html5ever dependency) ───────────────

/// Generate Telegram-compatible HTML from plain text + entities.
pub fn generate_html(text: &str, entities: &[tl::enums::MessageEntity]) -> String {
    use tl::enums::MessageEntity as ME;

    let mut markers: Vec<(i32, bool, String)> = Vec::new();

    for ent in entities {
        let (off, len, open, close) = match ent {
            ME::Bold(e)        => (e.offset, e.length, "<b>".into(),           "</b>".into()),
            ME::Italic(e)      => (e.offset, e.length, "<i>".into(),           "</i>".into()),
            ME::Underline(e)   => (e.offset, e.length, "<u>".into(),           "</u>".into()),
            ME::Strike(e)      => (e.offset, e.length, "<s>".into(),           "</s>".into()),
            ME::Spoiler(e)     => (e.offset, e.length, "<tg-spoiler>".into(),  "</tg-spoiler>".into()),
            ME::Code(e)        => (e.offset, e.length, "<code>".into(),        "</code>".into()),
            ME::Pre(e) => {
                let lang = if e.language.is_empty() { String::new() }
                           else { format!(" class=\"language-{}\"", e.language) };
                (e.offset, e.length, format!("<pre><code{lang}>"), "</code></pre>".into())
            }
            ME::TextUrl(e)     => (e.offset, e.length, format!("<a href=\"{}\">", escape_html(&e.url)), "</a>".into()),
            ME::MentionName(e) => (e.offset, e.length, format!("<a href=\"tg://user?id={}\">", e.user_id), "</a>".into()),
            ME::CustomEmoji(e) => (e.offset, e.length, format!("<tg-emoji emoji-id=\"{}\">", e.document_id), "</tg-emoji>".into()),
            _ => continue,
        };
        markers.push((off,       true,  open));
        markers.push((off + len, false, close));
    }

    markers.sort_by(|(a_pos, a_open, _), (b_pos, b_open, _)| {
        a_pos.cmp(b_pos).then_with(|| b_open.cmp(a_open))
    });

    let mut result = String::with_capacity(text.len() + markers.iter().map(|(_, _, s)| s.len()).sum::<usize>());
    let mut marker_idx = 0;
    let mut utf16_pos: i32 = 0;

    for ch in text.chars() {
        while marker_idx < markers.len() && markers[marker_idx].0 <= utf16_pos {
            result.push_str(&markers[marker_idx].2);
            marker_idx += 1;
        }
        match ch {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            c   => result.push(c),
        }
        utf16_pos += ch.len_utf16() as i32;
    }
    while marker_idx < markers.len() {
        result.push_str(&markers[marker_idx].2);
        marker_idx += 1;
    }

    result
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_bold() {
        let (text, ents) = parse_markdown("Hello **world**!");
        assert_eq!(text, "Hello world!");
        assert_eq!(ents.len(), 1);
        if let tl::enums::MessageEntity::Bold(b) = &ents[0] {
            assert_eq!(b.offset, 6);
            assert_eq!(b.length, 5);
        } else { panic!("expected bold"); }
    }

    #[test]
    fn markdown_inline_code() {
        let (text, ents) = parse_markdown("Use `foo()` to do it");
        assert_eq!(text, "Use foo() to do it");
        assert!(matches!(ents[0], tl::enums::MessageEntity::Code(_)));
    }

    #[test]
    fn html_bold_italic() {
        let (text, ents) = parse_html("<b>bold</b> and <i>italic</i>");
        assert_eq!(text, "bold and italic");
        assert_eq!(ents.len(), 2);
    }

    #[test]
    fn html_link() {
        let (text, ents) = parse_html("<a href=\"https://example.com\">click</a>");
        assert_eq!(text, "click");
        if let tl::enums::MessageEntity::TextUrl(e) = &ents[0] {
            assert_eq!(e.url, "https://example.com");
        } else { panic!("expected text url"); }
    }

    // HTML entity decoding is a hand-rolled-only feature; html5ever handles it natively.
    #[cfg(not(feature = "html5ever"))]
    #[test]
    fn html_entities_decoded() {
        let (text, _) = parse_html("A &amp; B &lt;3&gt;");
        assert_eq!(text, "A & B <3>");
    }

    #[test]
    fn generate_html_roundtrip() {
        let original = "Hello world";
        let entities = vec![tl::enums::MessageEntity::Bold(
            tl::types::MessageEntityBold { offset: 0, length: 5 })];
        let html = generate_html(original, &entities);
        assert_eq!(html, "<b>Hello</b> world");
        let (back, ents2) = parse_html(&html);
        assert_eq!(back, original);
        assert_eq!(ents2.len(), 1);
    }
}
