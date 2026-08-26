//! A small GBNF reader and generator, enough for the grammars in `docs/`.
//!
//! Supports the constructs those grammars use: rules, alternation, sequences,
//! groups, `*` `+` `?`, quoted terminals, character classes and `#` comments.

use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
enum Node {
    Seq(Vec<Node>),
    Alt(Vec<Node>),
    Rule(String),
    Literal(String),
    Class {
        negated: bool,
        ranges: Vec<(char, char)>,
    },
    Repeat {
        node: Box<Node>,
        min: usize,
        max: usize,
    },
}

pub struct Grammar {
    rules: HashMap<String, Node>,
    order: Vec<String>,
    /// Steps needed to reach a terminal, used to end recursion.
    costs: HashMap<String, usize>,
}

/// Characters a negated class may draw from.
const FALLBACK: &[char] = &['a', 'B', '7', '_', ' ', '.', '-'];

const MAX_REPEATS: usize = 3;
const MAX_OUTPUT: usize = 20_000;

impl Grammar {
    pub fn parse(source: &str) -> Result<Self, String> {
        let tokens = tokenize(source)?;
        let mut reader = Reader { tokens, pos: 0 };
        let mut rules = HashMap::new();
        let mut order = Vec::new();

        while let Some(name) = reader.next_rule_name()? {
            let body = reader.parse_alternation()?;
            if rules.insert(name.clone(), body).is_some() {
                return Err(format!("rule {name} is defined twice"));
            }
            order.push(name);
        }

        if !rules.contains_key("root") {
            return Err("no root rule".to_string());
        }

        let mut grammar = Grammar {
            rules,
            order,
            costs: HashMap::new(),
        };
        grammar.costs = grammar.compute_costs();
        Ok(grammar)
    }

    /// Rule names that are referenced but never defined.
    pub fn undefined_rules(&self) -> Vec<&String> {
        let mut missing: Vec<&String> = self
            .references()
            .into_iter()
            .filter(|name| !self.rules.contains_key(*name))
            .collect();
        missing.sort();
        missing.dedup();
        missing
    }

    /// Rule names that `root` can never reach.
    pub fn unreachable_rules(&self) -> Vec<&String> {
        let mut reached: HashSet<&str> = HashSet::new();
        let mut queue = vec!["root"];
        while let Some(name) = queue.pop() {
            if !reached.insert(name) {
                continue;
            }
            if let Some(node) = self.rules.get(name) {
                let mut names = Vec::new();
                collect_references(node, &mut names);
                queue.extend(names.into_iter().map(|n| n.as_str()));
            }
        }

        self.order
            .iter()
            .filter(|name| !reached.contains(name.as_str()))
            .collect()
    }

    /// Produce one document. The same seed always gives the same text.
    pub fn generate(&self, seed: u64) -> String {
        let mut rng = Rng::new(seed);
        let mut out = String::new();
        let root = &self.rules["root"];
        self.write(root, &mut rng, 24, &mut out);
        out
    }

    fn write(&self, node: &Node, rng: &mut Rng, budget: usize, out: &mut String) {
        if out.len() > MAX_OUTPUT {
            return;
        }
        match node {
            Node::Literal(text) => out.push_str(text),
            Node::Class { negated, ranges } => out.push(pick_char(*negated, ranges, rng)),
            Node::Seq(items) => {
                for item in items {
                    self.write(item, rng, budget, out);
                }
            }
            Node::Alt(choices) => {
                let choice = if budget == 0 {
                    // Take the way out that terminates soonest.
                    choices
                        .iter()
                        .min_by_key(|choice| self.cost_of(choice))
                        .unwrap_or(&choices[0])
                } else {
                    &choices[rng.below(choices.len())]
                };
                self.write(choice, rng, budget.saturating_sub(1), out);
            }
            Node::Repeat { node, min, max } => {
                let times = if budget == 0 {
                    *min
                } else {
                    let extra = (*max).min(MAX_REPEATS).saturating_sub(*min);
                    min + if extra == 0 { 0 } else { rng.below(extra + 1) }
                };
                for _ in 0..times {
                    self.write(node, rng, budget.saturating_sub(1), out);
                }
            }
            Node::Rule(name) => {
                if let Some(body) = self.rules.get(name) {
                    self.write(body, rng, budget.saturating_sub(1), out);
                }
            }
        }
    }

    /// Cheapest number of expansions to reach terminals.
    fn compute_costs(&self) -> HashMap<String, usize> {
        let mut costs: HashMap<String, usize> = HashMap::new();

        // Repeat until nothing improves; a rule that never resolves stays out.
        loop {
            let mut changed = false;
            for name in &self.order {
                let cost = cost(&self.rules[name], &costs);
                if let Some(cost) = cost {
                    let entry = costs.entry(name.clone()).or_insert(usize::MAX);
                    if cost + 1 < *entry {
                        *entry = cost + 1;
                        changed = true;
                    }
                }
            }
            if !changed {
                return costs;
            }
        }
    }

    fn cost_of(&self, node: &Node) -> usize {
        cost(node, &self.costs).unwrap_or(usize::MAX)
    }

    fn references(&self) -> Vec<&String> {
        let mut names = Vec::new();
        for body in self.rules.values() {
            collect_references(body, &mut names);
        }
        names
    }
}

/// Cost of a node, or None while some rule it needs is still unknown.
fn cost(node: &Node, costs: &HashMap<String, usize>) -> Option<usize> {
    match node {
        Node::Literal(_) | Node::Class { .. } => Some(1),
        Node::Rule(name) => costs.get(name).copied(),
        Node::Seq(items) => items
            .iter()
            .try_fold(0, |sum, item| Some(sum + cost(item, costs)?)),
        Node::Alt(choices) => choices.iter().filter_map(|c| cost(c, costs)).min(),
        Node::Repeat { node, min, .. } => {
            if *min == 0 {
                Some(0)
            } else {
                cost(node, costs).map(|c| c * min)
            }
        }
    }
}

fn collect_references<'a>(node: &'a Node, names: &mut Vec<&'a String>) {
    match node {
        Node::Rule(name) => names.push(name),
        Node::Seq(items) | Node::Alt(items) => {
            for item in items {
                collect_references(item, names);
            }
        }
        Node::Repeat { node, .. } => collect_references(node, names),
        Node::Literal(_) | Node::Class { .. } => {}
    }
}

fn pick_char(negated: bool, ranges: &[(char, char)], rng: &mut Rng) -> char {
    if negated {
        return *FALLBACK
            .iter()
            .filter(|c| !ranges.iter().any(|(lo, hi)| *lo <= **c && **c <= *hi))
            .nth(rng.below(FALLBACK.len()) % FALLBACK.len().max(1))
            .unwrap_or(&'x');
    }

    let (lo, hi) = ranges[rng.below(ranges.len())];
    let span = hi as u32 - lo as u32 + 1;
    char::from_u32(lo as u32 + rng.below(span as usize) as u32).unwrap_or(lo)
}

// ------------------------------------------------------------------ tokenizer

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Name(String),
    Define,
    Literal(String),
    Class {
        negated: bool,
        ranges: Vec<(char, char)>,
    },
    Open,
    Close,
    Pipe,
    Star,
    Plus,
    Question,
}

fn tokenize(source: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' | '\r' | '\n' => i += 1,
            '#' => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            ':' => {
                if chars.get(i..i + 3) == Some(&[':', ':', '=']) {
                    tokens.push(Token::Define);
                    i += 3;
                } else {
                    return Err(format!("stray ':' at byte {i}"));
                }
            }
            '"' => {
                let (text, next) = read_literal(&chars, i + 1)?;
                tokens.push(Token::Literal(text));
                i = next;
            }
            '[' => {
                let (negated, ranges, next) = read_class(&chars, i + 1)?;
                tokens.push(Token::Class { negated, ranges });
                i = next;
            }
            '(' => {
                tokens.push(Token::Open);
                i += 1;
            }
            ')' => {
                tokens.push(Token::Close);
                i += 1;
            }
            '|' => {
                tokens.push(Token::Pipe);
                i += 1;
            }
            '*' => {
                tokens.push(Token::Star);
                i += 1;
            }
            '+' => {
                tokens.push(Token::Plus);
                i += 1;
            }
            '?' => {
                tokens.push(Token::Question);
                i += 1;
            }
            c if c.is_alphanumeric() || c == '_' || c == '-' => {
                let start = i;
                while i < chars.len()
                    && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '-')
                {
                    i += 1;
                }
                tokens.push(Token::Name(chars[start..i].iter().collect()));
            }
            other => return Err(format!("unexpected '{other}' at byte {i}")),
        }
    }

    Ok(tokens)
}

fn read_literal(chars: &[char], mut i: usize) -> Result<(String, usize), String> {
    let mut text = String::new();
    while i < chars.len() {
        match chars[i] {
            '"' => return Ok((text, i + 1)),
            '\\' => {
                i += 1;
                text.push(unescape(*chars.get(i).ok_or("unterminated escape")?));
                i += 1;
            }
            c => {
                text.push(c);
                i += 1;
            }
        }
    }
    Err("unterminated literal".to_string())
}

fn read_class(chars: &[char], mut i: usize) -> Result<(bool, Vec<(char, char)>, usize), String> {
    let negated = chars.get(i) == Some(&'^');
    if negated {
        i += 1;
    }

    let mut ranges = Vec::new();
    while i < chars.len() {
        if chars[i] == ']' {
            return Ok((negated, ranges, i + 1));
        }

        let low = if chars[i] == '\\' {
            i += 1;
            unescape(*chars.get(i).ok_or("unterminated escape")?)
        } else {
            chars[i]
        };
        i += 1;

        if chars.get(i) == Some(&'-') && chars.get(i + 1) != Some(&']') {
            i += 1;
            let high = if chars[i] == '\\' {
                i += 1;
                unescape(*chars.get(i).ok_or("unterminated escape")?)
            } else {
                chars[i]
            };
            i += 1;
            ranges.push((low, high));
        } else {
            ranges.push((low, low));
        }
    }
    Err("unterminated character class".to_string())
}

fn unescape(c: char) -> char {
    match c {
        'n' => '\n',
        't' => '\t',
        'r' => '\r',
        other => other,
    }
}

// --------------------------------------------------------------------- reader

struct Reader {
    tokens: Vec<Token>,
    pos: usize,
}

impl Reader {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    /// True when a new rule starts here, which ends the rule being read.
    fn at_rule_start(&self) -> bool {
        matches!(self.peek(), Some(Token::Name(_)))
            && self.tokens.get(self.pos + 1) == Some(&Token::Define)
    }

    fn next_rule_name(&mut self) -> Result<Option<String>, String> {
        match self.peek() {
            None => Ok(None),
            Some(Token::Name(name)) => {
                let name = name.clone();
                self.pos += 2; // name and ::=
                Ok(Some(name))
            }
            Some(token) => Err(format!("expected a rule name, found {token:?}")),
        }
    }

    fn parse_alternation(&mut self) -> Result<Node, String> {
        let mut choices = vec![self.parse_sequence()?];
        while self.peek() == Some(&Token::Pipe) {
            self.pos += 1;
            choices.push(self.parse_sequence()?);
        }
        Ok(if choices.len() == 1 {
            choices.remove(0)
        } else {
            Node::Alt(choices)
        })
    }

    fn parse_sequence(&mut self) -> Result<Node, String> {
        let mut items = Vec::new();
        while let Some(token) = self.peek() {
            match token {
                Token::Pipe | Token::Close => break,
                Token::Name(_) if self.at_rule_start() => break,
                _ => items.push(self.parse_repeat()?),
            }
        }
        if items.is_empty() {
            return Err("empty alternative".to_string());
        }
        Ok(if items.len() == 1 {
            items.remove(0)
        } else {
            Node::Seq(items)
        })
    }

    fn parse_repeat(&mut self) -> Result<Node, String> {
        let atom = self.parse_atom()?;
        let (min, max) = match self.peek() {
            Some(Token::Star) => (0, usize::MAX),
            Some(Token::Plus) => (1, usize::MAX),
            Some(Token::Question) => (0, 1),
            _ => return Ok(atom),
        };
        self.pos += 1;
        Ok(Node::Repeat {
            node: Box::new(atom),
            min,
            max,
        })
    }

    fn parse_atom(&mut self) -> Result<Node, String> {
        match self.tokens.get(self.pos).cloned() {
            Some(Token::Name(name)) => {
                self.pos += 1;
                Ok(Node::Rule(name))
            }
            Some(Token::Literal(text)) => {
                self.pos += 1;
                Ok(Node::Literal(text))
            }
            Some(Token::Class { negated, ranges }) => {
                self.pos += 1;
                Ok(Node::Class { negated, ranges })
            }
            Some(Token::Open) => {
                self.pos += 1;
                let inner = self.parse_alternation()?;
                if self.peek() != Some(&Token::Close) {
                    return Err("missing ')'".to_string());
                }
                self.pos += 1;
                Ok(inner)
            }
            other => Err(format!("unexpected {other:?}")),
        }
    }
}

// ------------------------------------------------------------------------ rng

/// Deterministic, so a failing sample can be reproduced from its seed.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1))
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, limit: usize) -> usize {
        if limit == 0 {
            0
        } else {
            (self.next() % limit as u64) as usize
        }
    }
}
