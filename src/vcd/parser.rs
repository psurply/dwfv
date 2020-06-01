// SPDX-License-Identifier: MIT
use super::lexer::{Context, Keyword, Lexer, Token};
use crate::signaldb::{Scale, Signal, SignalDB, SignalValue, Timestamp};
use std::error::Error;
use std::fmt;
use std::io::prelude::*;

pub(crate) struct Parser<'a, I: BufRead> {
    lexer: Lexer<I>,
    signaldb: &'a SignalDB,
    scope: Vec<String>,
    limit: Option<i64>,
}

#[derive(Debug, PartialEq)]
pub(crate) struct SyntaxError {
    line: String,
}

impl Error for SyntaxError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Syntax Error: {:?}", self.line)
    }
}

macro_rules! syntax_error {
    ($parser: ident) => {
        Err(SyntaxError {
            line: $parser.lexer.get_current_line(),
        })
    };
}

macro_rules! expect_token {
    ($parser: ident, $ctx: expr, $pattern: pat, $block: block) => {{
        if let $pattern = $parser.lexer.pop($ctx) {
            $block
        } else {
            return syntax_error!($parser);
        }
    }};
}

impl<'a, I: BufRead> Parser<'a, I> {
    pub(crate) fn new(input: I, signaldb: &'a SignalDB) -> Parser<'a, I> {
        Parser {
            lexer: Lexer::new(input),
            signaldb,
            scope: Vec::new(),
            limit: None,
        }
    }

    fn parse_comment(&mut self) -> Result<(), SyntaxError> {
        loop {
            match self.lexer.pop(Context::Comment) {
                Token::Word(_) => (),
                Token::Keyword(Keyword::End) => break Ok(()),
                _ => break syntax_error!(self),
            }
        }
    }

    fn parse_scope(&mut self) -> Result<(), SyntaxError> {
        expect_token!(self, Context::Id, Token::Identifier(_scope_type), {
            expect_token!(self, Context::Id, Token::Identifier(scope_id), {
                expect_token!(self, Context::Stmt, Token::Keyword(Keyword::End), {
                    self.scope.push(scope_id);
                    let mut path = Vec::<&str>::new();
                    for scope in &self.scope {
                        path.push(scope);
                    }
                    self.signaldb.create_scope(&path);
                    Ok(())
                })
            })
        })
    }

    fn parse_upscope(&mut self) -> Result<(), SyntaxError> {
        expect_token!(self, Context::Stmt, Token::Keyword(Keyword::End), {
            self.scope.pop();
            Ok(())
        })
    }

    fn declare_new_var(&mut self, signal: Signal) {
        let mut path = Vec::<&str>::new();
        for scope in &self.scope {
            path.push(scope);
        }
        self.signaldb.declare_signal(&path, signal);
    }

    fn parse_var(&mut self) -> Result<(), SyntaxError> {
        expect_token!(self, Context::Id, Token::Identifier(_var_type), {
            expect_token!(self, Context::Id, Token::Integer(var_width), {
                expect_token!(
                    self,
                    Context::ShortId,
                    Token::Identifier(var_short_ident),
                    {
                        match self.lexer.pop(Context::Id) {
                            Token::Identifier(var_ident) => {
                                match self.lexer.pop(Context::IdRange) {
                                    Token::Range(_begin, _end) => expect_token!(
                                        self,
                                        Context::Stmt,
                                        Token::Keyword(Keyword::End),
                                        {
                                            self.declare_new_var(Signal::new(
                                                &var_short_ident,
                                                &var_ident,
                                                var_width,
                                            ));
                                            Ok(())
                                        }
                                    ),
                                    Token::Keyword(Keyword::End) => {
                                        self.declare_new_var(Signal::new(
                                            &var_short_ident,
                                            &var_ident,
                                            var_width,
                                        ));
                                        Ok(())
                                    }
                                    _ => syntax_error!(self),
                                }
                            }
                            Token::IdentifierRange(var_ident, _begin, _end) => {
                                expect_token!(self, Context::Stmt, Token::Keyword(Keyword::End), {
                                    self.declare_new_var(Signal::new(
                                        &var_short_ident,
                                        &var_ident,
                                        var_width,
                                    ));
                                    Ok(())
                                })
                            }
                            _ => syntax_error!(self),
                        }
                    }
                )
            })
        })
    }

    fn parse_value_change(&mut self, new_value: SignalValue) -> Result<(), SyntaxError> {
        expect_token!(self, Context::ShortId, Token::Identifier(ident), {
            self.signaldb
                .set_current_value(&ident, new_value)
                .map_err(|_err| SyntaxError {
                    line: self.lexer.get_current_line(),
                })
        })
    }

    fn parse_dumpvars(&mut self) -> Result<(), SyntaxError> {
        loop {
            match self.lexer.pop(Context::Value) {
                Token::Value(v) => self.parse_value_change(v)?,
                Token::ValueIdentifier(v, i) => {
                    self.signaldb
                        .set_current_value(&i, v)
                        .map_err(|_err| SyntaxError {
                            line: self.lexer.get_current_line(),
                        })?
                }
                Token::Keyword(Keyword::End) => {
                    self.signaldb.mark_as_initialized();
                    break Ok(());
                }
                _ => break syntax_error!(self),
            }
        }
    }

    fn parse_timescale(&mut self) -> Result<Timestamp, SyntaxError> {
        match self.lexer.pop(Context::Timescale) {
            Token::Integer(times) => {
                expect_token!(self, Context::Timescale, Token::Timescale(new_timescale), {
                    expect_token!(self, Context::Timescale, Token::Keyword(Keyword::End), {
                        Ok(new_timescale * times as i64)
                    })
                })
            }
            Token::Timescale(new_timescale) => {
                expect_token!(self, Context::Timescale, Token::Keyword(Keyword::End), {
                    Ok(new_timescale)
                })
            }
            _ => syntax_error!(self),
        }
    }

    pub(crate) fn set_limit(&mut self, timestamp: i64) {
        self.limit = Some(timestamp)
    }

    pub(crate) fn parse(&mut self) -> Result<(), SyntaxError> {
        let mut timescale = Timestamp::new(1, Scale::Picosecond);
        loop {
            match self.lexer.pop(Context::Stmt) {
                Token::Keyword(kw) => match kw {
                    Keyword::Comment | Keyword::Date | Keyword::Version => self.parse_comment()?,
                    Keyword::EndDefinitions => {
                        self.signaldb.mark_as_initialized();
                        self.parse_comment()?
                    }
                    Keyword::DumpVars => self.parse_dumpvars()?,
                    Keyword::Scope => self.parse_scope()?,
                    Keyword::Var => self.parse_var()?,
                    Keyword::Upscope => self.parse_upscope()?,
                    Keyword::Timescale => {
                        timescale = self.parse_timescale()?;
                        self.signaldb.set_timescale(timescale)
                    }
                    _ => break syntax_error!(self),
                },
                Token::Timestamp(v) => {
                    let t = timescale * v;
                    self.signaldb.set_time(t);
                    if let Some(limit) = self.limit {
                        if v > limit {
                            break Ok(());
                        }
                    }
                }
                Token::Value(v) => self.parse_value_change(v)?,
                Token::ValueIdentifier(v, i) => {
                    self.signaldb
                        .set_current_value(&i, v)
                        .map_err(|_err| SyntaxError {
                            line: self.lexer.get_current_line(),
                        })?
                }
                Token::Eof => break Ok(()),
                _ => break syntax_error!(self),
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::BufReader;

    #[test]
    fn header() {
        let input = BufReader::new(
            "
$comment
Any comment text.
$end"
                .as_bytes(),
        );
        let mut db = SignalDB::new();
        let mut p = Parser::new(input, &mut db);
        assert_eq!(p.parse(), Ok(()))
    }

    #[test]
    fn fail() {
        let input = BufReader::new("$end".as_bytes());
        let mut db = SignalDB::new();
        let mut p = Parser::new(input, &mut db);
        assert_eq!(
            p.parse(),
            Err(SyntaxError {
                line: String::from("$end")
            })
        )
    }

    #[test]
    fn full() {
        let input = BufReader::new(
            "
$date
   Date text. For example: November 11, 2009.
$end
$version
   VCD generator tool version info text.
$end
$comment
   Any comment text.
$end
$timescale 100ps $end
$scope module logic $end
$var wire 8 # data[7:0] $end
$var wire 8 # data_test [7:0] $end
$var wire 1 $ data_valid $end
$var wire 1 % en $end
$var wire 1 & rx_en $end
$var wire 1 ' tx_en $end
$var wire 1 ( empty $end
$var wire 1 ) underrun $end
$upscope $end
$enddefinitions $end
$dumpvars
bxxxxxxxx #
x$
0%
x&
x'
1(
0)
$end
#0
b10000001 #
0$
1%
0&
1'
0(
0)
#2211
0'
#2296
b0 #
1$
#2302
0$
#2303
"
            .as_bytes(),
        );
        let mut db = SignalDB::new();
        let mut p = Parser::new(input, &mut db);
        assert_eq!(p.parse(), Ok(()))
    }
}
