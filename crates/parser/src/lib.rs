use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use regex::Regex;
use std::sync::OnceLock;
use wasm_bindgen::prelude::*;

#[derive(Debug, Clone)]
struct Item {
    item_type: String,
    text: String,
}

enum ParseState {
    Idle,
    Heading {
        type_name: String,
        buffer: String,
    },
    Paragraph {
        buffer: String,
    },
    CodeBlock,
}

impl Item {
    fn new(item_type: &str, text: &str) -> Self {
        Self {
            item_type: item_type.to_string(),
            text: text.to_string(),
        }
    }

    fn to_luau(&self) -> String {
        let mut out = String::new();
        
        match self.item_type.as_str() {
            "Header1" | "Header2" => out.push_str(format!("    -- {}\n", self.text).as_str()),
            _ => {},
        }
        
        out.push_str(format!(
            r#"    {{ type = "{}", text = "{}" }},"#,
            self.item_type,
            escape_luau_string(&self.text)
        ).as_str());

        out
    }

    fn is_header(&self) -> bool {
        self.item_type.as_str() == "Header1" || self.item_type.as_str() == "Header2"
    }

    fn is_context(&self) -> bool {
        self.item_type.as_str() == "Context"
    }
}

fn escape_luau_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());

    for ch in s.chars() {
        match ch {
            '\\' => out.push_str(r"\\"),
            '\'' => out.push_str(r"'"),
            '\"' => out.push_str(r#"\""#),
            '\n' => out.push_str(r"\n"),
            '\r' => out.push_str(r"\r"),
            '\t' => out.push_str(r"\t"),
            _ => out.push(ch),
        }
    }

    out
}

fn normalize_fenced_blocks(markdown: &str) -> String {
    static OPEN_REGEX: OnceLock<Regex> = OnceLock::new();
    static CLOSE_REGEX: OnceLock<Regex> = OnceLock::new();

    let open_regex = OPEN_REGEX.get_or_init(|| Regex::new(r"```([^\n`])").unwrap());
    let close_regex = CLOSE_REGEX.get_or_init(|| Regex::new(r"([^\n`])```").unwrap());

    let step1 = open_regex.replace_all(markdown, "```\n$1");
    let step2 = close_regex.replace_all(&step1, "$1\n```");

    step2.into_owned()
}

#[wasm_bindgen]
pub fn convert(markdown: &str) -> String {
    let normalized = normalize_fenced_blocks(markdown);
    let items = parse_to_items(&normalized);
    let mut out = String::from("{\n");

    let mut last_item: Option<&Item> = None;

    for item in &items {
        if let Some(last_item) = last_item {
            if last_item.is_context() && item.is_header() {
                out.push('\n');
            }

            if last_item.is_header() && item.is_header() {
                out.push('\n');
            }
        }

        out.push_str(&item.to_luau());
        out.push('\n');

        last_item = Some(item);
    }

    out.push('}');
    out
}

fn parse_to_items(markdown: &str) -> Vec<Item> {
    let parser = Parser::new(markdown).into_offset_iter();
    let mut items: Vec<Item> = vec![];
    let mut state = ParseState::Idle;

    let mut last_block_end: usize = 0;

    for (event, range) in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                let type_name = match level {
                    HeadingLevel::H1 | HeadingLevel::H2 => "Header1",
                    HeadingLevel::H3 => "Header2",
                    _ => "Unknown",
                };
                state = ParseState::Heading {
                    type_name:type_name.to_string(),
                    buffer: String::new(),
                };
            }
            Event::End(TagEnd::Heading(_)) => {
                if let ParseState::Heading { type_name, buffer } = std::mem::replace(&mut state, ParseState::Idle) {
                    items.push(Item::new(&type_name, buffer.trim()));
                }

                last_block_end = range.end;
            }

            Event::Start(Tag::Paragraph) => {
                insert_blank_contexts(&mut items, markdown, last_block_end, range.start);
                state = ParseState::Paragraph { buffer: String::new() };
            }
            Event::End(TagEnd::Paragraph) => {
                if let ParseState::Paragraph { buffer } = std::mem::replace(&mut state, ParseState::Idle) {
                    if !buffer.is_empty() {
                        items.push(Item::new("Context", &buffer.trim()));
                    }
                }

                last_block_end = range.end;
            }

            Event::Start(Tag::CodeBlock(_)) => {
                insert_blank_contexts(&mut items, markdown, last_block_end, range.start);
                state = ParseState::CodeBlock;
            }
            Event::End(TagEnd::CodeBlock) => {
                state = ParseState::Idle;

                last_block_end = range.end;
            }

            Event::Text(text) => {
                match &mut state {
                    ParseState::Heading { buffer, .. } => {
                        buffer.push_str(&text);
                    }
                    ParseState::Paragraph { buffer } => {
                        buffer.push_str(&text);
                    }
                    ParseState::CodeBlock => {
                        for line in text.lines() {
                            items.push(Item::new("Context", line));
                        }
                    }
                    ParseState::Idle => {
                        items.push(Item::new("Unknown", &text));
                    }
                }
            }

            Event::Code(text) => {
                match &mut state {
                    ParseState::Paragraph { buffer } => {
                        if !buffer.trim().is_empty() {
                            items.push(Item::new("Context", buffer.trim()));
                            buffer.clear();
                        }

                        for line in text.lines() {
                            items.push(Item::new("Context", line));
                        }
                    }
                    ParseState::Heading { buffer, .. } => {
                        buffer.push_str(&text);
                    }
                    _ => {}
                }
            }

            Event::SoftBreak | Event::HardBreak => {
                if let ParseState::Paragraph { buffer } = &mut state {
                    buffer.push(' ');
                }
            }

            Event::Start(tag) => {
                insert_blank_contexts(&mut items, markdown, last_block_end, range.start);
                items.push(Item::new("Unknown", &format!("{:?}", tag)));
                last_block_end = range.end;
            }
            _ => {}
        }
    }

    items
}

fn insert_blank_contexts(items: &mut Vec<Item>, markdown: &str, start: usize, end: usize) {
    let blank_lines = count_blank_lines_between(markdown, start, end);
    if blank_lines == 0 { return }

    if let Some(last) = items.last() {
        if last.item_type == "Context" {
            for _ in 0..blank_lines {
                items.push(Item::new("Context", ""));
            }
        }
    }
}

fn count_blank_lines_between(text: &str, start: usize, end: usize) -> usize {
    if start >= end || end > text.len() {
        return 0;
    }

    let between = &text[start..end];
    between
        .lines()
        .filter(|line| line.trim().is_empty())
        .count()
}