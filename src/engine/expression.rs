use std::collections::HashMap;

use failure::{format_err, Error};
use nom::{
    branch::alt,
    bytes::complete::{tag, take_while, take_while_m_n},
    combinator::{map, opt, recognize},
    multi::many0,
    sequence::{pair, preceded},
    IResult,
};
use serde::{de, Deserialize, Deserializer};

use super::NodeState;

#[derive(Clone, Debug, PartialEq)]
pub enum Expression {
    Default,
    Manual,
    True,
    False,
    Node(String),
    NodeComplete(String),
    NodeDisabled(String),
    NodeUnlocked(String),
    And(Box<Expression>, Box<Expression>),
    Or(Box<Expression>, Box<Expression>),
    Not(Box<Expression>),
}

impl Default for Expression {
    fn default() -> Self {
        Expression::Default
    }
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

// Recognizes the first part of a node id.
//   Matches regexp `[a-z][a-z0-9]*`
fn node_first_part(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        take_while_m_n(1, 1, is_lower_alpha),
        opt(take_while(is_lower_alphanum)),
    ))(input)
}

// Recognizes node id parts following the first.
//   Matches regexp `-[a-z0-9]+`
fn node_part(input: &str) -> IResult<&str, &str> {
    recognize(pair(tag("-"), take_while(is_lower_alphanum)))(input)
}

fn node_id(input: &str) -> IResult<&str, &str> {
    preceded(
        whitespace,
        recognize(pair(node_first_part, many0(node_part))),
    )(input)
}

fn node(input: &str) -> IResult<&str, Expression> {
    map(node_id, |s: &str| Expression::Node(s.into()))(input)
}

fn node_complete(input: &str) -> IResult<&str, Expression> {
    let (input, _) = preceded(whitespace, tag("complete"))(input)?;
    let (input, _) = preceded(whitespace, tag("("))(input)?;
    let (input, expr) = map(preceded(whitespace, node_id), |s: &str| {
        Expression::NodeComplete(s.into())
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
    alt((parenthetical, not, node_complete, node))(input)
}

fn and_expr(input: &str) -> IResult<&str, Expression> {
    let (input, a) = preceded(whitespace, operand)(input)?;
    let (input, _) = preceded(whitespace, tag("&&"))(input)?;
    let (input, b) = preceded(whitespace, and)(input)?;

    Ok((input, Expression::And(Box::new(a), Box::new(b))))
}

fn and(input: &str) -> IResult<&str, Expression> {
    alt((and_expr, operand))(input)
}

fn or_expr(input: &str) -> IResult<&str, Expression> {
    let (input, a) = preceded(whitespace, and)(input)?;
    let (input, _) = preceded(whitespace, tag("||"))(input)?;
    let (input, b) = preceded(whitespace, or)(input)?;

    Ok((input, Expression::Or(Box::new(a), Box::new(b))))
}

fn or(input: &str) -> IResult<&str, Expression> {
    alt((or_expr, and))(input)
}

// Param is a special case for parameter nodes.  It is way of making
// a node enabled manually instead of defaulting to true.
fn param(input: &str) -> IResult<&str, Expression> {
    let (input, _) = tag("param")(input)?;
    Ok((input, Expression::Manual))
}

fn parse_expression(input: &str) -> IResult<&str, Expression> {
    alt((param, or))(input)
}

impl Expression {
    pub fn parse(input: &str) -> Result<Expression, Error> {
        parse_expression(input)
            .map(|v| v.1)
            .map_err(|e| format_err!("error parsing expression: {}", e))
    }

    pub fn eval_default(self, default_value: Expression) -> Expression {
        if self == Expression::Default {
            default_value
        } else {
            self
        }
    }

    pub fn or(self, other: Self) -> Expression {
        // short circut constants
        if self == Expression::False {
            other
        } else if other == Expression::False {
            self
        } else if other == Expression::True || self == Expression::True {
            Expression::True
        } else {
            Expression::Or(Box::new(self), Box::new(other))
        }
    }

    pub fn and(self, other: Self) -> Expression {
        // short circut constants
        if self == Expression::True {
            other
        } else if other == Expression::True {
            self
        } else if other == Expression::False || self == Expression::False {
            Expression::False
        } else {
            Expression::And(Box::new(self), Box::new(other))
        }
    }

    // Return a `Vec` of node ids upon which this expression depends.
    pub fn deps(&self) -> Vec<String> {
        match self {
            Expression::Default | Expression::Manual | Expression::False | Expression::True => {
                vec![]
            }
            Expression::Node(id)
            | Expression::NodeComplete(id)
            | Expression::NodeDisabled(id)
            | Expression::NodeUnlocked(id) => vec![id.clone()],
            Expression::Not(obj) => obj.deps(),
            Expression::And(a, b) => {
                let mut d = a.deps();
                d.append(&mut b.deps());
                d
            }
            Expression::Or(a, b) => {
                let mut d = a.deps();
                d.append(&mut b.deps());
                d
            }
        }
    }

    fn find_state<'a>(
        id: &String,
        state: &'a HashMap<String, NodeState>,
    ) -> Result<&'a NodeState, Error> {
        state.get(id).ok_or(format_err!("can't find id {}", id))
    }

    // Evaluate this expression based on `state`.
    pub fn evaluate_by(
        &self,
        state: &HashMap<String, NodeState>,
        threshold: &NodeState,
    ) -> Result<bool, Error> {
        match self {
            Expression::Default => Err(format_err!("evaluate called on default expression")),
            Expression::Manual => Err(format_err!("evaluate called on manual expression")),
            Expression::False => Ok(false),
            Expression::True => Ok(true),
            Expression::Node(id) => Self::find_state(id, state).map(|o| o.at_least(threshold)),
            Expression::NodeComplete(id) => {
                Self::find_state(id, state).map(|o| o.is(&NodeState::Complete))
            }
            Expression::NodeDisabled(id) => {
                Self::find_state(id, state).map(|o| o.is(&NodeState::Disabled))
            }
            Expression::NodeUnlocked(id) => {
                Self::find_state(id, state).map(|o| o.is(&NodeState::Unlocked))
            }
            Expression::Not(obj) => obj.evaluate_by(state, threshold).map(|v| !v),
            Expression::And(a, b) => {
                Ok(a.evaluate_by(state, threshold)? && b.evaluate_by(state, threshold)?)
            }
            Expression::Or(a, b) => {
                Ok(a.evaluate_by(state, threshold)? || b.evaluate_by(state, threshold)?)
            }
        }
    }

    pub fn evaluate_unlocked(&self, state: &HashMap<String, NodeState>) -> Result<bool, Error> {
        self.evaluate_by(state, &NodeState::Unlocked)
    }

    pub fn evaluate_enabled(&self, state: &HashMap<String, NodeState>) -> Result<bool, Error> {
        self.evaluate_by(state, &NodeState::Locked)
    }
}

impl<'de> Deserialize<'de> for Expression {
    fn deserialize<D>(deserializer: D) -> Result<Expression, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = <String>::deserialize(deserializer)?;
        Expression::parse(&s).map_err(de::Error::custom)
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
    fn node_names() {
        test_expression("a", Expression::Node("a".into()));
        test_expression("aa", Expression::Node("aa".into()));
        test_expression("a0", Expression::Node("a0".into()));
        test_expression("a0-b1-2", Expression::Node("a0-b1-2".into()));
        test_expression("a0-b1-c2", Expression::Node("a0-b1-c2".into()));
        test_expression(" a0-b1-c2", Expression::Node("a0-b1-c2".into()));

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
        test_expressions(&vec!["param"], Expression::Manual);

        test_expressions(
            &vec!["complete(tower-key)", " complete ( tower-key )"],
            Expression::NodeComplete("tower-key".into()),
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
            Expression::Not(Box::new(Expression::Node("tower-key".into()))),
        );

        test_expressions(
            &vec![
                "tower-key && luca-key",
                "(tower-key && luca-key)",
                "(tower-key) && (luca-key)",
            ],
            Expression::And(
                Box::new(Expression::Node("tower-key".into())),
                Box::new(Expression::Node("luca-key".into())),
            ),
        );

        test_expressions(
            &vec![
                "tower-key && !luca-key",
                "(tower-key && !luca-key)",
                "(tower-key) && !(luca-key)",
            ],
            Expression::And(
                Box::new(Expression::Node("tower-key".into())),
                Box::new(Expression::Not(Box::new(Expression::Node(
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
                Box::new(Expression::Node("tower-key".into())),
                Box::new(Expression::Node("luca-key".into())),
            ),
        );

        test_expressions(
            &vec!["tower-key && magma-key && luca-key"],
            Expression::And(
                Box::new(Expression::Node("tower-key".into())),
                Box::new(Expression::And(
                    Box::new(Expression::Node("magma-key".into())),
                    Box::new(Expression::Node("luca-key".into())),
                )),
            ),
        );

        test_expressions(
            &vec!["tower-key && magma-key || luca-key"],
            Expression::Or(
                Box::new(Expression::And(
                    Box::new(Expression::Node("tower-key".into())),
                    Box::new(Expression::Node("magma-key".into())),
                )),
                Box::new(Expression::Node("luca-key".into())),
            ),
        );

        test_expressions(
            &vec!["complete(hook) || complete(magma-key)"],
            Expression::Or(
                Box::new(Expression::NodeComplete("hook".into())),
                Box::new(Expression::NodeComplete("magma-key".into())),
            ),
        );
    }

    #[test]
    fn deps() {
        assert_eq!(
            Expression::parse("complete(hook) || complete(magma-key)")
                .unwrap()
                .deps(),
            vec!["hook".to_string(), "magma-key".to_string()]
        );

        assert_eq!(
            Expression::parse("complete(hook) || !complete(magma-key)")
                .unwrap()
                .deps(),
            vec!["hook".to_string(), "magma-key".to_string()]
        );

        assert_eq!(
            Expression::parse("tower-key && complete(magma-key) || !luca-key")
                .unwrap()
                .deps(),
            vec![
                "tower-key".to_string(),
                "magma-key".to_string(),
                "luca-key".to_string()
            ]
        );
    }
}
