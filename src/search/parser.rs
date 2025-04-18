// SPDX-License-Identifier: MIT

/// Grammar of the search expressions:
///
/// ```ebnf
/// expr =
///     expr, "or", expr_tier
///     | expr_tier
///     ;
///
/// expr_tier =
///     expr_tier, "and", expr_term
///     | expr_tier, "nand", expr_term
///     ;
///
/// expr_term =
///     left_value, equal, right_value
///     | left_value, not_equal, right_value
///     | left_value, transition, right_value
///     | "after" dec_value
///     | "before" dec_value
///     | left_value
///     | "(" expr ")"
///     ;
///
/// equal = "is" | "equals" | "=";
/// not_equal = "is not", "!=";
/// transition = "becomes", "<-";
///
/// left_value = id;
/// right_value =
///     literal_value
///     | left_value
///     | "(" right_value ")"
///     ;
///
/// literal_value =
///     dec_value
///     | bin_value
///     | hex_value
///     ;
///
/// id = \$[[:graph:]]+;
/// bin_value = b[01uzw-]+;
/// hex_value = h[0-9A-Fa-f]+;
/// dec_value = [0-9]+;
/// ```
use super::expr::{ExprAst, ValueAst};
use crate::signaldb::SignalValue;
use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{tag, take, take_while, take_while_m_n, take_while1},
    combinator::{opt, recognize},
    error::Error,
    sequence::{delimited, pair, preceded, separated_pair},
};
use std::str::FromStr;

// Check functions

fn is_digit_start(input: char) -> bool {
    input.is_ascii_digit() && input != '0'
}

fn is_digit(input: char) -> bool {
    input.is_ascii_digit()
}

fn is_binary_digit(input: char) -> bool {
    matches!(input, '0' | '1' | 'u' | 'z' | 'w' | '-')
}

fn is_hex_digit(input: char) -> bool {
    input.is_ascii_hexdigit()
}

fn is_identifier(input: char) -> bool {
    !(input.is_whitespace() || input.is_control())
}

// Combinators

/// Call a parser with optional whitespace on either side.
fn token<'a, O, F>(parser: F) -> impl Parser<&'a str, Output = O, Error = Error<&'a str>>
where
    F: Fn(&'a str) -> IResult<&'a str, O>,
{
    delimited(opt(whitespace), parser, opt(whitespace))
}

// Parsers

/// Recognize whitespace.
fn whitespace(input: &str) -> IResult<&str, &str> {
    take_while1(char::is_whitespace)(input)
}

/// Recognize an expression.
pub(crate) fn expr(input: &str) -> IResult<&str, ExprAst> {
    alt((or, tier)).parse(input)
}

/// Recognize a tiered expression.
fn tier(input: &str) -> IResult<&str, ExprAst> {
    alt((and, nand, term)).parse(input)
}

/// Recognize an expression term.
fn term(input: &str) -> IResult<&str, ExprAst> {
    alt((parens, equal, not_equal, transition, before, after, any)).parse(input)
}

/// Recognize an expression in parentheses.
fn parens(input: &str) -> IResult<&str, ExprAst> {
    delimited(tag("("), expr, tag(")")).parse(input)
}

/// Recognize a number.
fn number(input: &str) -> IResult<&str, ValueAst> {
    recognize(alt((
        pair(tag("b"), take_while1(is_binary_digit)),
        pair(tag("h"), take_while1(is_hex_digit)),
        pair(tag("0"), take(0_usize)),
        pair(take_while_m_n(1, 1, is_digit_start), take_while(is_digit)),
    )))
    .parse(input)
    .map(|(rest, value)| {
        let value = match &value[..1] {
            "b" => SignalValue::from_str(&value[1..]).unwrap(),
            "h" => SignalValue::from_hex(&value[1..]),
            _ => SignalValue::new(value.parse().unwrap()),
        };

        (rest, ValueAst::Literal(value))
    })
}

/// Recognize a decimal number.
fn decimal(input: &str) -> IResult<&str, i64> {
    recognize(alt((
        pair(tag("0"), take(0_usize)),
        pair(take_while_m_n(1, 1, is_digit_start), take_while(is_digit)),
    )))
    .parse(input)
    .map(|(rest, value)| (rest, value.parse().unwrap()))
}

/// Recognize an identifier.
fn identifier(input: &str) -> IResult<&str, ValueAst> {
    preceded(tag("$"), take_while1(is_identifier))
        .parse(input)
        .map(|(rest, id)| (rest, ValueAst::Id(id.to_string())))
}

/// Recognize a value in parentheses.
fn value_parens(input: &str) -> IResult<&str, ValueAst> {
    delimited(tag("("), value, tag(")")).parse(input)
}

/// Recognize a value.
fn value(input: &str) -> IResult<&str, ValueAst> {
    alt((number, identifier, value_parens)).parse(input)
}

/// Recognize an equivalence condition.
fn equal(input: &str) -> IResult<&str, ExprAst> {
    separated_pair(
        token(identifier),
        alt((tag("="), tag("is"), tag("equals"))),
        token(value),
    )
    .parse(input)
    .map(|(rest, (left, right))| {
        let left = match left {
            ValueAst::Id(id) => id,
            _ => unreachable!(),
        };

        (rest, ExprAst::Equal(left, right))
    })
}

/// Recognize a non-equivalence condition.
fn not_equal(input: &str) -> IResult<&str, ExprAst> {
    separated_pair(
        token(identifier),
        alt((tag("!="), tag("is not"))),
        token(value),
    )
    .parse(input)
    .map(|(rest, (left, right))| {
        let left = match left {
            ValueAst::Id(id) => id,
            _ => unreachable!(),
        };

        (rest, ExprAst::Not(Box::new(ExprAst::Equal(left, right))))
    })
}

/// Recognize a transition.
fn transition(input: &str) -> IResult<&str, ExprAst> {
    separated_pair(
        token(identifier),
        alt((tag("<-"), tag("becomes"))),
        token(value),
    )
    .parse(input)
    .map(|(rest, (left, right))| {
        let left = match left {
            ValueAst::Id(id) => id,
            _ => unreachable!(),
        };

        (rest, ExprAst::Transition(left, right))
    })
}

/// Recognize any transition.
fn any(input: &str) -> IResult<&str, ExprAst> {
    token(identifier).parse(input).map(|(rest, value)| {
        let value = match value {
            ValueAst::Id(id) => id,
            _ => unreachable!(),
        };

        (rest, ExprAst::AnyTransition(value))
    })
}

/// Recognize a logical and.
fn and(input: &str) -> IResult<&str, ExprAst> {
    separated_pair(token(term), tag("and"), token(tier))
        .parse(input)
        .map(|(rest, (left, right))| (rest, ExprAst::And(Box::new(left), Box::new(right))))
}

/// Recognize a logical nand.
fn nand(input: &str) -> IResult<&str, ExprAst> {
    separated_pair(token(term), tag("nand"), token(tier))
        .parse(input)
        .map(|(rest, (left, right))| {
            let value = ExprAst::And(Box::new(left), Box::new(right));

            (rest, ExprAst::Not(Box::new(value)))
        })
}

/// Recognize a logical or.
fn or(input: &str) -> IResult<&str, ExprAst> {
    separated_pair(token(term), tag("or"), token(tier))
        .parse(input)
        .map(|(rest, (left, right))| (rest, ExprAst::Or(Box::new(left), Box::new(right))))
}

/// Recognize an after duration.
fn after(input: &str) -> IResult<&str, ExprAst> {
    preceded(token(tag("after")), decimal)
        .parse(input)
        .map(|(rest, value)| (rest, ExprAst::After(value)))
}

/// Recognize a before duration.
fn before(input: &str) -> IResult<&str, ExprAst> {
    preceded(token(tag("before")), decimal)
        .parse(input)
        .map(|(rest, value)| (rest, ExprAst::Before(value)))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::signaldb::BitValue::{self, High, HighZ, Low, Undefined};
    use nom::Err;
    use nom::error::ErrorKind::{Tag, TakeWhileMN};
    use nom::error::{Error, ErrorKind};

    fn make_error<Output>(input: &str, code: ErrorKind) -> IResult<&str, Output> {
        Err(Err::Error(Error { input, code }))
    }

    fn make_id(id: &str) -> ValueAst {
        ValueAst::Id(id.to_string())
    }

    fn make_literal(value: u64) -> ValueAst {
        ValueAst::Literal(SignalValue::new(value))
    }

    fn make_bitvalue(width: usize, value: BitValue) -> ValueAst {
        ValueAst::Literal(SignalValue::new_default(width, value))
    }

    #[test]
    fn test_number() {
        assert_eq!(number("b0 foo"), Ok((" foo", make_bitvalue(1, Low))));
        assert_eq!(number("bz bar"), Ok((" bar", make_bitvalue(1, HighZ))));
        assert_eq!(number("bu"), Ok(("", make_bitvalue(1, Undefined))));
        assert_eq!(number("b12"), Ok(("2", make_bitvalue(1, High))));
        assert_eq!(number("h0"), Ok(("", make_literal(0))));
        assert_eq!(number("h4a"), Ok(("", make_literal(74))));
        assert_eq!(number("0"), Ok(("", make_literal(0))));
        assert_eq!(number("1"), Ok(("", make_literal(1))));
        assert_eq!(number("01"), Ok(("1", make_literal(0))));
        assert_eq!(number("42"), Ok(("", make_literal(42))));

        assert_eq!(number(""), make_error("", TakeWhileMN));
        assert_eq!(number(" "), make_error(" ", TakeWhileMN));
        assert_eq!(number("b2"), make_error("b2", TakeWhileMN));
        assert_eq!(number("$a"), make_error("$a", TakeWhileMN));
    }

    #[test]
    fn test_identifier() {
        assert_eq!(identifier("$a"), Ok(("", make_id("a"))));
        assert_eq!(identifier("$abc"), Ok(("", make_id("abc"))));
        assert_eq!(identifier("$.*"), Ok(("", make_id(".*"))));

        assert_eq!(identifier(""), make_error("", Tag));
        assert_eq!(identifier(" "), make_error(" ", Tag));
        assert_eq!(identifier("0"), make_error("0", Tag));
        assert_eq!(identifier("a"), make_error("a", Tag));
    }

    #[test]
    fn test_value() {
        assert_eq!(value("$foo123 bar"), Ok((" bar", make_id("foo123"))));
        assert_eq!(value("hdz"), Ok(("z", make_literal(13))));

        assert_eq!(value(""), make_error("", Tag));
        assert_eq!(value(" "), make_error(" ", Tag));
        assert_eq!(value("a"), make_error("a", Tag));
    }

    #[test]
    fn test_equal() {
        assert_eq!(
            equal("$a = b0 bar"),
            Ok((
                "bar",
                ExprAst::Equal("a".to_string(), make_bitvalue(1, Low))
            ))
        );
        assert_eq!(
            equal("$abc is 42z"),
            Ok(("z", ExprAst::Equal("abc".to_string(), make_literal(42))))
        );
        assert_eq!(
            equal("$.* equals bu"),
            Ok((
                "",
                ExprAst::Equal(".*".to_string(), make_bitvalue(1, Undefined))
            ))
        );

        assert_eq!(equal(""), make_error("", Tag));
        assert_eq!(equal(" "), make_error("", Tag));
        assert_eq!(equal("bz = bz"), make_error("bz = bz", Tag));
        assert_eq!(equal("foo = bar"), make_error("foo = bar", Tag));
    }

    #[test]
    fn test_not_equal() {
        assert_eq!(
            not_equal("$a != b0 bar"),
            Ok((
                "bar",
                ExprAst::Not(Box::new(ExprAst::Equal(
                    "a".to_string(),
                    make_bitvalue(1, Low)
                )))
            ))
        );
        assert_eq!(
            not_equal("$.* is not bu"),
            Ok((
                "",
                ExprAst::Not(Box::new(ExprAst::Equal(
                    ".*".to_string(),
                    make_bitvalue(1, Undefined)
                )))
            ))
        );

        assert_eq!(not_equal(""), make_error("", Tag));
        assert_eq!(not_equal(" "), make_error("", Tag));
        assert_eq!(not_equal("bz != bz"), make_error("bz != bz", Tag));
        assert_eq!(not_equal("foo != bar"), make_error("foo != bar", Tag));
    }

    #[test]
    fn test_transition() {
        assert_eq!(
            transition("$a <- b0 bar"),
            Ok((
                "bar",
                ExprAst::Transition("a".to_string(), make_bitvalue(1, Low))
            ))
        );
        assert_eq!(
            transition("$.* becomes bu"),
            Ok((
                "",
                ExprAst::Transition(".*".to_string(), make_bitvalue(1, Undefined))
            ))
        );

        assert_eq!(transition(""), make_error("", Tag));
        assert_eq!(transition(" "), make_error("", Tag));
        assert_eq!(transition("bz <- bz"), make_error("bz <- bz", Tag));
        assert_eq!(
            transition("foo becomes bar"),
            make_error("foo becomes bar", Tag)
        );
    }

    #[test]
    fn test_any() {
        assert_eq!(
            any("$a foo"),
            Ok(("foo", ExprAst::AnyTransition("a".to_string())))
        );
        assert_eq!(
            any("$.*"),
            Ok(("", ExprAst::AnyTransition(".*".to_string())))
        );

        assert_eq!(any(""), make_error("", Tag));
        assert_eq!(any(" "), make_error("", Tag));
    }

    #[test]
    fn test_after() {
        assert_eq!(after("after 12 foo"), Ok((" foo", ExprAst::After(12))));

        assert_eq!(after(""), make_error("", Tag));
        assert_eq!(after(" "), make_error("", Tag));
    }

    #[test]
    fn test_before() {
        assert_eq!(before("before 2"), Ok(("", ExprAst::Before(2))));
        assert_eq!(before("before 23 foo"), Ok((" foo", ExprAst::Before(23))));

        assert_eq!(before(""), make_error("", Tag));
        assert_eq!(before(" "), make_error("", Tag));
    }

    #[test]
    fn test_and() {
        assert_eq!(
            and("$a = 8 and before 2"),
            Ok((
                "",
                ExprAst::And(
                    Box::new(ExprAst::Equal("a".to_string(), make_literal(8))),
                    Box::new(ExprAst::Before(2))
                )
            ))
        );
        assert_eq!(
            and("$a <- 0 and $b = 4"),
            Ok((
                "",
                ExprAst::And(
                    Box::new(ExprAst::Transition("a".to_string(), make_literal(0))),
                    Box::new(ExprAst::Equal("b".to_string(), make_literal(4)))
                )
            ))
        );

        assert_eq!(and(""), make_error("", Tag));
        assert_eq!(and(" "), make_error("", Tag));
    }

    #[test]
    fn test_or() {
        assert_eq!(
            or("$a = 8 or before 2"),
            Ok((
                "",
                ExprAst::Or(
                    Box::new(ExprAst::Equal("a".to_string(), make_literal(8))),
                    Box::new(ExprAst::Before(2))
                )
            ))
        );

        assert_eq!(or(""), make_error("", Tag));
        assert_eq!(or(" "), make_error("", Tag));
    }

    #[test]
    fn test_expr() {
        assert_eq!(
            expr("($a becomes (0))"),
            Ok(("", ExprAst::Transition("a".to_string(), make_literal(0)),))
        );
        assert_eq!(
            expr("$a <- 0 and $b = 4"),
            Ok((
                "",
                ExprAst::And(
                    Box::new(ExprAst::Transition("a".to_string(), make_literal(0))),
                    Box::new(ExprAst::Equal("b".to_string(), make_literal(4)))
                )
            ))
        );

        assert_eq!(or(""), make_error("", Tag));
        assert_eq!(or(" "), make_error("", Tag));
    }
}
