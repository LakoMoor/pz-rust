//! Parser for Project Zomboid's brace-structured script files (`media/scripts/**/*.txt`).
//!
//! ```text
//! module Base { item Axe { Weight = 2.5, Tags = base:a;base:b, component Foo { X = 1 } } }
//! ```
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    /// `Key = Value`
    Prop(String, String),
    /// A statement with neither `=` nor a body, e.g. a recipe input line `item 1 tags[base:x] mode:keep`.
    Line(String),
    Block(Block),
}

/// `kind [name...] { entries }`, e.g. `item Axe { .. }` or `module Base { .. }`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Block {
    pub kind: String,
    pub name: String,
    pub entries: Vec<Entry>,
}

impl Block {
    pub fn prop(&self, key: &str) -> Option<&str> {
        self.entries.iter().find_map(|e| match e { Entry::Prop(k, v) if k == key => Some(v.as_str()), _ => None })
    }
    pub fn blocks<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = &'a Block> {
        self.entries.iter().filter_map(move |e| match e { Entry::Block(b) if b.kind == kind => Some(b), _ => None })
    }
}

#[derive(Debug)]
pub struct ParseError { pub offset: usize, pub msg: &'static str }

struct P<'a> { s: &'a [u8], i: usize }

impl<'a> P<'a> {
    fn skip_ws(&mut self) {
        loop {
            while self.i < self.s.len() && (self.s[self.i].is_ascii_whitespace() || self.s[self.i] == 0xEF || self.s[self.i] == 0xBB || self.s[self.i] == 0xBF) { self.i += 1; }
            if self.s[self.i..].starts_with(b"//") {
                while self.i < self.s.len() && self.s[self.i] != b'\n' { self.i += 1; }
            } else if self.s[self.i..].starts_with(b"/*") {
                self.i += 2;
                while self.i < self.s.len() && !self.s[self.i..].starts_with(b"*/") { self.i += 1; }
                self.i = (self.i + 2).min(self.s.len());
            } else { return; }
        }
    }

    fn entries(&mut self, top: bool) -> Result<Vec<Entry>, ParseError> {
        let mut out = Vec::new();
        loop {
            self.skip_ws();
            if self.i >= self.s.len() {
                return if top { Ok(out) } else { Err(ParseError { offset: self.i, msg: "unclosed block" }) };
            }
            match self.s[self.i] {
                b'}' => {
                    if top { return Err(ParseError { offset: self.i, msg: "unmatched }" }); }
                    self.i += 1;
                    return Ok(out);
                }
                b',' => { self.i += 1; }
                _ => out.push(self.statement()?),
            }
        }
    }

    /// Read text up to a `,` `{` `}` or end of line; the delimiter is not consumed.
    fn head(&mut self) -> String {
        let st = self.i;
        while self.i < self.s.len() && !matches!(self.s[self.i], b',' | b'{' | b'}' | b'\n' | b'\r') { self.i += 1; }
        String::from_utf8_lossy(&self.s[st..self.i]).trim().to_string()
    }

    fn statement(&mut self) -> Result<Entry, ParseError> {
        let h = self.head();
        // A body may start on the next line: `item Axe \n {`.
        let save = self.i;
        self.skip_ws();
        if self.i < self.s.len() && self.s[self.i] == b'{' {
            self.i += 1;
            let mut words = h.split_whitespace();
            let kind = words.next().unwrap_or("").to_string();
            let name = words.collect::<Vec<_>>().join(" ");
            return Ok(Entry::Block(Block { kind, name, entries: self.entries(false)? }));
        }
        self.i = save;
        Ok(match h.split_once('=') {
            Some((k, v)) => Entry::Prop(k.trim().to_string(), v.trim().to_string()),
            None => Entry::Line(h),
        })
    }
}

pub fn parse(src: &str) -> Result<Vec<Entry>, ParseError> {
    P { s: src.as_bytes(), i: 0 }.entries(true)
}

/// All blocks of `kind` nested anywhere (modules contain the actual definitions).
pub fn find_all<'a>(entries: &'a [Entry], kind: &str, out: &mut Vec<&'a Block>) {
    for e in entries {
        if let Entry::Block(b) = e {
            if b.kind == kind { out.push(b); }
            find_all(&b.entries, kind, out);
        }
    }
}

pub fn parse_file(path: impl AsRef<Path>) -> std::io::Result<Result<Vec<Entry>, ParseError>> {
    Ok(parse(&String::from_utf8_lossy(&std::fs::read(path)?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_blocks_props_and_lines() {
        let src = "module Base {\n item Axe\n {\n Weight = 2.5,\n Tags = a;b,\n // note\n component Foo { X = 1 }\n }\n craftRecipe R { inputs { item 1 tags[x] mode:keep, } }\n}";
        let e = parse(src).unwrap();
        let mut items = vec![];
        find_all(&e, "item", &mut items);
        assert_eq!(items[0].name, "Axe");
        assert_eq!(items[0].prop("Weight"), Some("2.5"));
        assert_eq!(items[0].blocks("component").next().unwrap().prop("X"), Some("1"));
        let mut inputs = vec![];
        find_all(&e, "inputs", &mut inputs);
        assert_eq!(inputs[0].entries, vec![Entry::Line("item 1 tags[x] mode:keep".into())]);
    }
}
