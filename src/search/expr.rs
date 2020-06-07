// SPDX-License-Identifier: MIT
use crate::signaldb::SignalValue;
use lalrpop_util::lalrpop_mod;
use std::error::Error;
use std::io;

lalrpop_mod!(parser, "/search/parser.rs");

#[derive(Debug, PartialEq, Eq)]
pub enum ValueAst {
    Literal(SignalValue),
    Id(String),
}

#[derive(Debug, PartialEq, Eq)]
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
    pub(crate) fn from_str(expr: &str) -> Result<ExprAst, Box<dyn Error>> {
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
    use crate::signaldb::BitValue;

    #[test]
    fn expr() {
        assert!(ExprAst::from_str("$a is b0").is_ok());
        assert!(ExprAst::from_str("$abc is 42").is_ok());
        assert!(ExprAst::from_str("$abc is bu").is_ok());
        assert!(ExprAst::from_str("$u is bz").is_ok());
        assert!(ExprAst::from_str("$bz is bz").is_ok());
        assert!(ExprAst::from_str("bz = bz").is_err());
        assert!(ExprAst::from_str("$.* is bz").is_ok());
        assert!(ExprAst::from_str("$a becomes 0").is_ok());
        assert!(ExprAst::from_str("$a becomes (0)").is_ok());
        assert!(ExprAst::from_str("($a becomes (0))").is_ok());
        assert!(ExprAst::from_str("$a <- 0 and $b = 4").is_ok());
        assert!(ExprAst::from_str("$a <- 0 and after 42").is_ok());
        assert!(ExprAst::from_str("$a <- 0 and before 42").is_ok());
        assert!(ExprAst::from_str("$a <- 0").is_ok());
        assert!(ExprAst::from_str("$a <- $b").is_ok());
    }

    #[test]
    fn test_eq() {
        assert_eq!(
            ExprAst::from_str("$a is b0").unwrap(),
            ExprAst::Equal("a".to_string(), ValueAst::Literal(SignalValue::new(0)))
        );

        assert_eq!(
            ExprAst::from_str("$abc is 42").unwrap(),
            ExprAst::Equal("abc".to_string(), ValueAst::Literal(SignalValue::new(42)))
        );

        assert_eq!(
            ExprAst::from_str("$abc is bu").unwrap(),
            ExprAst::Equal("abc".to_string(), ValueAst::Literal(SignalValue::invalid()))
        );

        assert_eq!(
            ExprAst::from_str("$u is bz").unwrap(),
            ExprAst::Equal(
                "u".to_string(),
                ValueAst::Literal(SignalValue::new_default(1, BitValue::HighZ)),
            )
        );

        assert_eq!(
            ExprAst::from_str("$bz is bz").unwrap(),
            ExprAst::Equal(
                "bz".to_string(),
                ValueAst::Literal(SignalValue::new_default(1, BitValue::HighZ)),
            )
        );

        assert_eq!(
            ExprAst::from_str("$.* is bz").unwrap(),
            ExprAst::Equal(
                ".*".to_string(),
                ValueAst::Literal(SignalValue::new_default(1, BitValue::HighZ)),
            )
        );
    }

    #[test]
    fn test_transition() {
        assert_eq!(
            ExprAst::from_str("$a becomes 0").unwrap(),
            ExprAst::Transition("a".to_string(), ValueAst::Literal(SignalValue::new(0)))
        );

        assert_eq!(
            ExprAst::from_str("$a becomes (0)").unwrap(),
            ExprAst::Transition("a".to_string(), ValueAst::Literal(SignalValue::new(0)))
        );

        assert_eq!(
            ExprAst::from_str("($a becomes (0))").unwrap(),
            ExprAst::Transition("a".to_string(), ValueAst::Literal(SignalValue::new(0)))
        );

        assert_eq!(
            ExprAst::from_str("$a <- 0").unwrap(),
            ExprAst::Transition("a".to_string(), ValueAst::Literal(SignalValue::new(0)))
        );

        assert_eq!(
            ExprAst::from_str("$a <- $b").unwrap(),
            ExprAst::Transition("a".to_string(), ValueAst::Id("b".to_string()))
        );
    }

    #[test]
    fn test_condition() {
        assert_eq!(
            ExprAst::from_str("$a <- 0 and $b = 4").unwrap(),
            ExprAst::And(
                Box::new(ExprAst::Transition(
                    "a".to_string(),
                    ValueAst::Literal(SignalValue::new(0)),
                )),
                Box::new(ExprAst::Equal(
                    "b".to_string(),
                    ValueAst::Literal(SignalValue::new(4)),
                )),
            )
        );

        assert_eq!(
            ExprAst::from_str("$a <- 0 and after 42").unwrap(),
            ExprAst::And(
                Box::new(ExprAst::Transition(
                    "a".to_string(),
                    ValueAst::Literal(SignalValue::new(0)),
                )),
                Box::new(ExprAst::After(42)),
            )
        );

        assert_eq!(
            ExprAst::from_str("$a <- 0 and before 42").unwrap(),
            ExprAst::And(
                Box::new(ExprAst::Transition(
                    "a".to_string(),
                    ValueAst::Literal(SignalValue::new(0)),
                )),
                Box::new(ExprAst::Before(42)),
            )
        );
    }
}
