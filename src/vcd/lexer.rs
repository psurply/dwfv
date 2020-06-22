// SPDX-License-Identifier: MIT
use crate::signaldb::{Scale, SignalValue, Timestamp};
use std::collections::VecDeque;
use std::io::prelude::*;
use std::str::FromStr;

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum Keyword {
    Comment,
    Date,
    DumpVars,
    End,
    EndDefinitions,
    Scope,
    Timescale,
    Var,
    Version,
    Upscope,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Token {
    Word(String),
    Keyword(Keyword),
    Range(u64, u64),
    Identifier(String),
    IdentifierRange(String, u64, u64),
    Integer(usize),
    Value(SignalValue),
    ValueIdentifier(SignalValue, String),
    Timestamp(i64),
    Timescale(Timestamp),
    Eof,
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum Context {
    Comment,
    Stmt,
    Id,
    IdRange,
    ShortId,
    Value,
    Timescale,
}

pub(crate) struct Lexer<I: BufRead> {
    pub(crate) buf: String,
    input: I,
    tok_queue: VecDeque<Token>,
}

impl Token {
    fn retokenize_kw(word: &str) -> Option<Token> {
        let kw = match word {
            "$comment" => Some(Keyword::Comment),
            "$date" => Some(Keyword::Date),
            "$dumpvars" => Some(Keyword::DumpVars),
            "$end" => Some(Keyword::End),
            "$enddefinitions" => Some(Keyword::EndDefinitions),
            "$scope" => Some(Keyword::Scope),
            "$timescale" => Some(Keyword::Timescale),
            "$var" => Some(Keyword::Var),
            "$version" => Some(Keyword::Version),
            "$upscope" => Some(Keyword::Upscope),
            _ => None,
        };
        kw.map(Token::Keyword)
    }

    fn retokenize_integer(word: &str) -> Option<Token> {
        match word.parse() {
            Ok(i) => Some(Token::Integer(i)),
            Err(_) => None,
        }
    }

    fn retokenize_value(word: &str) -> Option<Token> {
        match word.chars().next().unwrap() {
            'b' => Some(Token::Value(SignalValue::from_str(&word[1..]).unwrap())),
            'x' | '-' | 'z' | 'u' | 'w' | '1' | '0' => Some(Token::ValueIdentifier(
                SignalValue::from_str(&word[..1]).unwrap(),
                word[1..].to_string(),
            )),
            's' => Some(Token::Value(SignalValue::from_symbol_str(&word[1..]))),
            _ => None,
        }
    }

    fn retokenize_timestamp(word: &str) -> Option<Token> {
        match word.chars().next().unwrap() {
            '#' => match word[1..].parse() {
                Ok(i) => Some(Token::Timestamp(i)),
                Err(_) => None,
            },
            _ => None,
        }
    }

    fn retokenize_range(word: &str) -> Option<Token> {
        if !word.starts_with('[') || !word.ends_with(']') {
            return None;
        }

        let mut iter = word[1..word.len() - 1].split(':');
        let start = iter.next()?.parse().ok()?;
        let end = iter.next()?.parse().ok()?;

        if iter.next().is_some() {
            return None;
        }

        Some(Token::Range(start, end))
    }

    fn retokenize_id_range(word: &str) -> Option<Token> {
        for (i, c) in word.chars().enumerate() {
            if c == '[' {
                if let Some(Token::Range(begin, end)) = Token::retokenize_range(&word[i..]) {
                    return Some(Token::IdentifierRange(word[..i].to_string(), begin, end));
                } else {
                    return None;
                }
            }
        }
        Some(Token::Identifier(word.to_string()))
    }

    fn retokenize_timescale(word: &str) -> Option<Token> {
        if !word.ends_with("ms")
            && !word.ends_with("us")
            && !word.ends_with("ns")
            && !word.ends_with("ps")
            && !word.ends_with("fs")
            && !word.ends_with('s')
        {
            return None;
        }

        let end = word.chars().position(|ch| !ch.is_numeric())?;

        Some(Token::Timescale(Timestamp::new(
            word[..end].parse().unwrap_or(1),
            Scale::from_str(&word[end..]).unwrap(),
        )))
    }

    fn retokenize(self, ctx: Context) -> Token {
        match self {
            Token::Word(word) => match ctx {
                Context::Comment => {
                    Token::retokenize_kw(&word).unwrap_or_else(|| Token::Word(word.to_string()))
                }
                Context::Stmt => Token::retokenize_kw(&word)
                    .or_else(|| Token::retokenize_timestamp(&word))
                    .or_else(|| Token::retokenize_value(&word))
                    .unwrap_or(Token::Word(word)),
                Context::Id => Token::retokenize_integer(&word)
                    .or_else(|| Token::retokenize_id_range(&word))
                    .unwrap_or(Token::Identifier(word)),
                Context::ShortId => Token::Identifier(word),
                Context::IdRange => Token::retokenize_range(&word)
                    .or_else(|| Token::retokenize_kw(&word))
                    .unwrap_or(Token::Identifier(word)),
                Context::Value => Token::retokenize_kw(&word)
                    .or_else(|| Token::retokenize_value(&word))
                    .unwrap_or(Token::Word(word)),
                Context::Timescale => Token::retokenize_kw(&word)
                    .or_else(|| Token::retokenize_integer(&word))
                    .or_else(|| Token::retokenize_timescale(&word))
                    .unwrap_or(Token::Word(word)),
            },
            tok => tok,
        }
    }
}

impl<I: BufRead> Lexer<I> {
    pub(crate) fn new(input: I) -> Lexer<I> {
        Lexer {
            input,
            buf: String::new(),
            tok_queue: VecDeque::new(),
        }
    }

    fn feed_words(&mut self) {
        self.buf.clear();
        let num_bytes = {
            loop {
                let num_bytes = self.input.read_line(&mut self.buf);
                if self.buf != "\n" {
                    break num_bytes;
                }
            }
        };
        match num_bytes {
            Ok(0) => self.tok_queue.push_back(Token::Eof),
            Ok(_) => {
                for word in self.buf.split_whitespace() {
                    self.tok_queue.push_back(Token::Word(word.to_string()))
                }
            }
            Err(e) => panic!("Error while reading input file: {:?}", e),
        }
    }

    fn prepare_queue(&mut self) {
        if self.tok_queue.is_empty() {
            self.feed_words()
        }
    }

    pub(crate) fn pop(&mut self, ctx: Context) -> Token {
        loop {
            self.prepare_queue();
            if let Some(tok) = self.tok_queue.pop_front() {
                return tok.retokenize(ctx)
            }
        }
    }

    pub(crate) fn get_current_line(&self) -> String {
        self.buf.to_string()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::BufReader;

    #[test]
    fn plain() {
        let input = BufReader::new("Hello World".as_bytes());
        let mut l = Lexer::new(input);
        assert_eq!(l.pop(Context::Stmt), Token::Word("Hello".to_string()));
        assert_eq!(l.pop(Context::Stmt), Token::Word("World".to_string()));
        assert_eq!(l.pop(Context::Stmt), Token::Eof);
    }

    #[test]
    fn keywords() {
        let input = BufReader::new("Hello $world $end".as_bytes());
        let mut l = Lexer::new(input);
        assert_eq!(l.pop(Context::Stmt), Token::Word("Hello".to_string()));
        assert_eq!(l.pop(Context::Stmt), Token::Word("$world".to_string()));
        assert_eq!(l.pop(Context::Stmt), Token::Keyword(Keyword::End));
        assert_eq!(l.pop(Context::Stmt), Token::Eof);
    }

    #[test]
    fn empty_lines() {
        let input = BufReader::new("   \n$end".as_bytes());
        let mut l = Lexer::new(input);
        assert_eq!(l.pop(Context::Stmt), Token::Keyword(Keyword::End));
        assert_eq!(l.pop(Context::Stmt), Token::Eof);
    }
}
