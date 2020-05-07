// SPDX-License-Identifier: MIT
use crate::signaldb::SignalValue;
use std::error::Error;
use std::io;

lalrpop_mod!(parser, "/search/parser.rs");

#[derive(Debug)]
pub enum ValueAst {
    Literal(SignalValue),
    Id(String),
}

#[derive(Debug)]
pub enum ExprAst {
    Equal(String, ValueAst),
    Transition(String, ValueAst),
    AnyTransition(String),
    Not(Box<ExprAst>),
    And(Box<ExprAst>, Box<ExprAst>),
    Or(Box<ExprAst>, Box<ExprAst>),
    After(i64),
    Before(i64),
}

impl ExprAst {
    pub fn from_str(expr: &str) -> Result<ExprAst, Box<dyn Error>> {
        let ast = parser::ExprParser::new().parse(expr).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Syntax Error: {:?}", err),
            )
        })?;
        Ok(ast)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn expr() {
        assert!(parser::ExprParser::new().parse("$a is b0").is_ok());
        assert!(parser::ExprParser::new().parse("$abc is 42").is_ok());
        assert!(parser::ExprParser::new().parse("$abc is bu").is_ok());
        assert!(parser::ExprParser::new().parse("$u is bz").is_ok());
        assert!(parser::ExprParser::new().parse("$bz is bz").is_ok());
        assert!(parser::ExprParser::new().parse("bz = bz").is_err());
        assert!(parser::ExprParser::new().parse("$.* is bz").is_ok());
        assert!(parser::ExprParser::new().parse("$a becomes 0").is_ok());
        assert!(parser::ExprParser::new().parse("$a becomes (0)").is_ok());
        assert!(parser::ExprParser::new().parse("($a becomes (0))").is_ok());
        assert!(parser::ExprParser::new()
            .parse("$a <- 0 and $b = 4")
            .is_ok());
        assert!(parser::ExprParser::new()
            .parse("$a <- 0 and after 42")
            .is_ok());
        assert!(parser::ExprParser::new()
            .parse("$a <- 0 and before 42")
            .is_ok());
        assert!(parser::ExprParser::new().parse("$a <- 0").is_ok());
        assert!(parser::ExprParser::new().parse("$a <- $b").is_ok());
    }
}
