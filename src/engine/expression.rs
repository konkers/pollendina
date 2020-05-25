use failure::{format_err, Error};
use nom::{
    branch::alt,
    bytes::complete::{tag, take_while, take_while_m_n},
    combinator::{map, opt, recognize},
    multi::{many0},
    sequence::{pair, preceded},
    IResult,
};

#[derive(Clone, Debug, PartialEq)]
pub enum Expression {
    Objective(String),
    ObjectiveComplete(String),
    And(Box<Expression>, Box<Expression>),
    Or(Box<Expression>, Box<Expression>),
    Not(Box<Expression>),
}

fn is_lower_alphanum(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit()
}

fn is_lower_alpha(c: char) -> bool {
    c.is_ascii_lowercase()
}

fn whitespace(input: &str) -> IResult<&str, &str> {
    let chars = " \t\r\n";
    take_while(move |c| chars.contains(c))(input)
}

// Recognizes the first part of an objective id.
//   Matches regexp `[a-z][a-z0-9]*`
fn objective_first_part(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        take_while_m_n(1, 1, is_lower_alpha),
        opt(take_while(is_lower_alphanum)),
    ))(input)
}

// Recognizes objectice id parts following the first.
//   Matches regexp `-[a-z0-9]+`
fn objective_part(input: &str) -> IResult<&str, &str> {
    recognize(pair(tag("-"), take_while(is_lower_alphanum)))(input)
}

fn objective_id(input: &str) -> IResult<&str, &str> {
    preceded(
        whitespace,
        recognize(pair(objective_first_part, many0(objective_part))),
    )(input)
}

fn objective(input: &str) -> IResult<&str, Expression> {
    map(objective_id, |s: &str| Expression::Objective(s.into()))(input)
}

fn objective_complete(input: &str) -> IResult<&str, Expression> {
    let (input, _) = preceded(whitespace, tag("complete"))(input)?;
    let (input, _) = preceded(whitespace, tag("("))(input)?;
    let (input, expr) = map(preceded(whitespace, objective_id), |s: &str| {
        Expression::ObjectiveComplete(s.into())
    })(input)?;
    let (input, _) = preceded(whitespace, tag(")"))(input)?;

    Ok((input, expr))
}

fn parenthetical(input: &str) -> IResult<&str, Expression> {
    let (input, _) = preceded(whitespace, tag("("))(input)?;
    let (input, expr) = preceded(whitespace, parse_expression)(input)?;
    let (input, _) = preceded(whitespace, tag(")"))(input)?;
    Ok((input, expr))
}

fn not(input: &str) -> IResult<&str, Expression> {
    let (input, _) = preceded(whitespace, tag("!"))(input)?;
    map(preceded(whitespace, parse_expression), |e: Expression| {
        Expression::Not(Box::new(e))
    })(input)
}

fn operand(input: &str) -> IResult<&str, Expression> {
    alt((parenthetical, not, objective_complete, objective))(input)
}

fn or_expr(input: &str) -> IResult<&str, Expression> {
    let (input, a) = preceded(whitespace, operand)(input)?;
    let (input, _) = preceded(whitespace, tag("||"))(input)?;
    let (input, b) = preceded(whitespace, or)(input)?;

    Ok((input, Expression::Or(Box::new(a), Box::new(b))))
}

fn or(input: &str) -> IResult<&str, Expression> {
    alt((or_expr, operand))(input)
}

fn and_expr(input: &str) -> IResult<&str, Expression> {
    let (input, a) = preceded(whitespace, or)(input)?;
    let (input, _) = preceded(whitespace, tag("&&"))(input)?;
    let (input, b) = preceded(whitespace, and)(input)?;

    Ok((input, Expression::And(Box::new(a), Box::new(b))))
}

fn and(input: &str) -> IResult<&str, Expression> {
    alt((and_expr, or))(input)
}

fn parse_expression(input: &str) -> IResult<&str, Expression> {
    and(input)
}

impl Expression {
    pub fn parse(input: &str) -> Result<Expression, Error> {
        parse_expression(input)
            .map(|v| v.1)
            .map_err(|e| format_err!("error parsing expression: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_expression(s: &str, e: Expression) {
        println!("TEST == {}", s);
        assert_eq!(parse_expression(s), Ok(("", e)));
    }

    fn test_expressions(exprs: &Vec<&str>, e: Expression) {
        for s in exprs {
            println!("TEST == {}", s);
            assert_eq!(parse_expression(s), Ok(("", e.clone())));
        }
    }

    #[test]
    fn objective_names() {
        test_expression("a", Expression::Objective("a".into()));
        test_expression("aa", Expression::Objective("aa".into()));
        test_expression("a0", Expression::Objective("a0".into()));
        test_expression("a0-b1-2", Expression::Objective("a0-b1-2".into()));
        test_expression("a0-b1-c2", Expression::Objective("a0-b1-c2".into()));
        test_expression(" a0-b1-c2", Expression::Objective("a0-b1-c2".into()));

        assert_eq!(
            parse_expression("0"),
            Err(nom::Err::Error(("0", nom::error::ErrorKind::TakeWhileMN)))
        );

        assert_eq!(
            parse_expression("-"),
            Err(nom::Err::Error(("-", nom::error::ErrorKind::TakeWhileMN)))
        );

        /*
        assert_eq!(
            parse_expression("a-"),
            Err(nom::Err::Error(("a-", nom::error::ErrorKind::TakeWhileMN)))
        );
        */

        assert_eq!(
            parse_expression("0a"),
            Err(nom::Err::Error(("0a", nom::error::ErrorKind::TakeWhileMN)))
        );

        assert_eq!(
            parse_expression("-a"),
            Err(nom::Err::Error(("-a", nom::error::ErrorKind::TakeWhileMN)))
        );
    }

    #[test]
    fn expressions() {
        test_expressions(
            &vec!["tower-key", " tower-key"],
            Expression::Objective("tower-key".into()),
        );
        test_expressions(
            &vec!["complete(tower-key)", " complete ( tower-key )"],
            Expression::ObjectiveComplete("tower-key".into()),
        );
        test_expressions(
            &vec![
                "!tower-key",
                " !tower-key",
                "! tower-key",
                " ! tower-key",
                "( ! tower-key)",
                "!(tower-key)",
            ],
            Expression::Not(Box::new(Expression::Objective("tower-key".into()))),
        );

        test_expressions(
            &vec![
                "tower-key && luca-key",
                "(tower-key && luca-key)",
                "(tower-key) && (luca-key)",
            ],
            Expression::And(
                Box::new(Expression::Objective("tower-key".into())),
                Box::new(Expression::Objective("luca-key".into())),
            ),
        );

        test_expressions(
            &vec![
                "tower-key && !luca-key",
                "(tower-key && !luca-key)",
                "(tower-key) && !(luca-key)",
            ],
            Expression::And(
                Box::new(Expression::Objective("tower-key".into())),
                Box::new(Expression::Not(Box::new(Expression::Objective(
                    "luca-key".into(),
                )))),
            ),
        );

        test_expressions(
            &vec![
                "tower-key || luca-key",
                "(tower-key || luca-key)",
                "(tower-key) || (luca-key)",
            ],
            Expression::Or(
                Box::new(Expression::Objective("tower-key".into())),
                Box::new(Expression::Objective("luca-key".into())),
            ),
        );

        test_expressions(
            &vec!["tower-key && magma-key && luca-key"],
            Expression::And(
                Box::new(Expression::Objective("tower-key".into())),
                Box::new(Expression::And(
                    Box::new(Expression::Objective("magma-key".into())),
                    Box::new(Expression::Objective("luca-key".into())),
                )),
            ),
        );

        test_expressions(
            &vec!["tower-key || magma-key && luca-key"],
            Expression::And(
                Box::new(Expression::Or(
                    Box::new(Expression::Objective("tower-key".into())),
                    Box::new(Expression::Objective("magma-key".into())),
                )),
                Box::new(Expression::Objective("luca-key".into())),
            ),
        );

        test_expressions(
            &vec![
                "complete(hook) || complete(magma-key)",
            ],
            Expression::Or(
                Box::new(Expression::ObjectiveComplete("hook".into())),
                Box::new(Expression::ObjectiveComplete("magma-key".into())),
            ),
        );
    }
}
