use crate::ast::Statement;
use crate::dialect::{Dialect, GenericDialect};
use crate::keywords::Keyword;
use crate::parser::{Parser, ParserError};
use crate::tokenizer::Token;

#[derive(Debug, Default)]
pub struct CypherDialect;

impl Dialect for CypherDialect {
    fn is_identifier_start(&self, ch: char) -> bool {
        ch.is_alphabetic() || ch == '_' || ch == '#' || ch == '@'
    }

    fn is_identifier_part(&self, ch: char) -> bool {
        ch.is_alphabetic()
            || ch.is_ascii_digit()
            || ch == '@'
            || ch == '$'
            || ch == '#'
            || ch == '_'
    }

    fn parse_statement(&self, parser: &mut Parser) -> Option<Result<Statement, ParserError>> {
        // MATCH keyword
        if parser.parse_keyword(Keyword::MATCH) {
            return Some(parse_cypher_match(parser));
        }

        // Fall back to default sql parsing
        None
    }
}

// Parse Cypher MATCH statement:
fn parse_cypher_match(parser: &mut Parser) -> Result<Statement, ParserError> {
    // Parse: (p[:Label])
    parser.expect_token(&Token::LParen)?;

    let var_ident = parser.parse_identifier()?;
    let var = var_ident.value.clone();

    // Optional :Label
    let mut label: Option<String> = None;
    if let Token::Colon = parser.peek_token().token {
        parser.next_token(); // consume ':'
        let label_ident = parser.parse_identifier()?;
        label = Some(label_ident.value.clone());
    }

    parser.expect_token(&Token::RParen)?;

    // RETURN
    if !parser.parse_keyword(Keyword::RETURN) {
        return Err(ParserError::ParserError(
            "Expected RETURN in Cypher MATCH statement".into(),
        ));
    }

    // Decide which RETURN we’re dealing with:
    let next = parser.peek_token();
    if let Token::Word(w) = &next.token {
        if w.keyword == Keyword::COUNT {
            // MATCH (...) RETURN count(p) AS alias [LIMIT n]
            return parse_cypher_return_count(parser, var, label);
        }
    }

    // Otherwise, treat it as a properties/labels RETURN:
    // MATCH (p) RETURN labels(p) AS labels, p.name AS name, p.age AS age LIMIT 10;
    // MATCH (p:Person) RETURN p.name AS name, p.age AS age LIMIT 10;
    parse_cypher_return_props(parser, var, label)
}

fn parse_cypher_return_count(
    parser: &mut Parser,
    var: String,
    label: Option<String>,
) -> Result<Statement, ParserError> {
    use Token::*;

    // COUNT (p) [AS alias] [LIMIT n]
    let next = parser.peek_token();
    if let Word(w) = &next.token {
        if w.keyword != Keyword::COUNT {
            return Err(ParserError::ParserError(
                "Expected COUNT in RETURN clause".into(),
            ));
        }
    } else {
        return Err(ParserError::ParserError(
            "Expected COUNT in RETURN clause".into(),
        ));
    }
    parser.next_token(); // consume COUNT

    // (p)
    parser.expect_token(&LParen)?;
    let count_var_ident = parser.parse_identifier()?;
    let count_var = count_var_ident.value.clone();
    parser.expect_token(&RParen)?;

    if count_var != var {
        return Err(ParserError::ParserError(
            "COUNT() variable must match MATCH() variable".into(),
        ));
    }

    // AS alias (optional)
    let alias = if parser.parse_keyword(Keyword::AS) {
        let alias_ident = parser.parse_identifier()?;
        alias_ident.value.clone()
    } else {
        // default when user writes: MATCH (p) RETURN count(p);
        "count".to_string()
    };

    // Optional LIMIT n
    let mut limit_sql = String::new();
    if parser.parse_keyword(Keyword::LIMIT) {
        let tok = parser.next_token();
        let n = match tok.token {
            Number(ref s, _) => s.clone(),
            _ => {
                return Err(ParserError::ParserError(
                    "Expected numeric literal after LIMIT".into(),
                ))
            }
        };
        limit_sql = format!(" LIMIT {}", n);
    }

    // Build SQL
    let base = if let Some(label) = label {
        format!(
            "SELECT COUNT(*) AS {alias} \
             FROM nodes AS {var} \
             WHERE {var}.label = '{label}'"
        )
    } else {
        format!("SELECT COUNT(*) AS {alias} FROM nodes AS {var}")
    };

    let sql = format!("{base}{limit_sql}");

    // Re-parse with GenericDialect
    let generic = GenericDialect;
    let mut sql_parser = Parser::new(&generic).try_with_sql(&sql)?;
    let mut stmts = sql_parser.parse_statements()?;

    if stmts.len() != 1 {
        return Err(ParserError::ParserError(format!(
            "Cypher COUNT translation produced {} SQL statements, expected 1",
            stmts.len()
        )));
    }

    Ok(stmts.remove(0))
}

fn parse_cypher_return_props(
    parser: &mut Parser,
    var: String,
    label: Option<String>,
) -> Result<Statement, ParserError> {
    use Token::*;

    let mut select_items: Vec<String> = Vec::new();

    loop {
        let next = parser.peek_token();

        match &next.token {
            // labels(p) [AS alias]
            Word(w)
                if w.keyword == Keyword::NoKeyword && w.value.eq_ignore_ascii_case("labels") =>
            {
                parser.next_token(); // consume 'labels'

                parser.expect_token(&LParen)?;
                let inner_ident = parser.parse_identifier()?;
                let inner_var = inner_ident.value.clone();
                parser.expect_token(&RParen)?;

                if inner_var != var {
                    return Err(ParserError::ParserError(
                        "labels() variable must match MATCH() variable".into(),
                    ));
                }

                // Optional AS alias
                let mut alias = "labels".to_string();
                if parser.parse_keyword(Keyword::AS) {
                    let alias_ident = parser.parse_identifier()?;
                    alias = alias_ident.value.clone();
                }

                // labels(p) -> p.label (single label)
                let expr_sql = format!("{var}.label AS {alias}");
                select_items.push(expr_sql);
            }

            // Either p.prop [AS alias]  OR  bare p [AS alias]
            Word(_) => {
                // consume the identifier (should be the MATCH var)
                let var_ident = parser.parse_identifier()?;
                let var_name = var_ident.value.clone();

                if var_name != var {
                    return Err(ParserError::ParserError(
                        "Only simple p, labels(p), or p.prop expressions are supported in RETURN clause"
                            .into(),
                    ));
                }

                // Look at what comes after 'p'
                // Clone the token so we don't hold a reference into the parser's buffer.
                let after_tok = parser.peek_token().token.clone();

                match after_tok {
                    // p.prop [AS alias]
                    Period => {
                        parser.next_token(); // consume '.'

                        let prop_ident = parser.parse_identifier()?;
                        let prop_name = prop_ident.value.clone();

                        // Optional AS alias
                        let mut alias = prop_name.clone();
                        if parser.parse_keyword(Keyword::AS) {
                            let alias_ident = parser.parse_identifier()?;
                            alias = alias_ident.value.clone();
                        }

                        let expr_sql = format!(
                            "json_extract({var_name}.properties, '$.{prop_name}') AS {alias}"
                        );
                        select_items.push(expr_sql);
                    }

                    // Bare node: p [AS alias]
                    // e.g., MATCH (p:Person) RETURN p;
                    // or   MATCH (p:Person) RETURN p AS person;
                    Word(w) if w.keyword == Keyword::AS => {
                        // consume AS
                        let _ = parser.parse_keyword(Keyword::AS);
                        let alias_ident = parser.parse_identifier()?;
                        let alias = alias_ident.value.clone();

                        let expr_sql = format!(
                            "json_object('id', {var_name}.id, 'label', {var_name}.label, 'properties', {var_name}.properties) AS {alias}"
                        );
                        select_items.push(expr_sql);
                    }

                    // Bare node followed by LIMIT (handle LIMIT after the loop)
                    Word(w) if w.keyword == Keyword::LIMIT => {
                        let alias = var_name.clone();
                        let expr_sql = format!(
                            "json_object('id', {var_name}.id, 'label', {var_name}.label, 'properties', {var_name}.properties) AS {alias}"
                        );
                        select_items.push(expr_sql);
                    }

                    // Bare node at end of list / statement: ',', ';', EOF
                    Comma | SemiColon | EOF => {
                        let alias = var_name.clone();
                        let expr_sql = format!(
                            "json_object('id', {var_name}.id, 'label', {var_name}.label, 'properties', {var_name}.properties) AS {alias}"
                        );
                        select_items.push(expr_sql);
                    }

                    _ => {
                        return Err(ParserError::ParserError(
                            "Unexpected token after variable in RETURN clause; expected '.', AS, ',', or LIMIT"
                                .into(),
                        ));
                    }
                }
            }

            _ => {
                return Err(ParserError::ParserError(
                    "Unexpected token in RETURN clause".into(),
                ));
            }
        }

        // If next token is a comma, continue loop
        if let Comma = parser.peek_token().token {
            parser.next_token(); // consume ','
            continue;
        }

        break;
    }

    // Optional LIMIT n
    let mut limit_sql = String::new();
    if parser.parse_keyword(Keyword::LIMIT) {
        let tok = parser.next_token();
        let n = match tok.token {
            Number(ref s, _) => s.clone(),
            _ => {
                return Err(ParserError::ParserError(
                    "Expected numeric literal after LIMIT".into(),
                ))
            }
        };
        limit_sql = format!(" LIMIT {}", n);
    }

    let select_list = select_items.join(", ");

    let base = if let Some(label) = label {
        format!(
            "SELECT {select_list} \
             FROM nodes AS {var} \
             WHERE {var}.label = '{label}'"
        )
    } else {
        format!("SELECT {select_list} FROM nodes AS {var}")
    };

    let sql = format!("{base}{limit_sql}");

    let generic = GenericDialect;
    let mut sql_parser = Parser::new(&generic).try_with_sql(&sql)?;
    let mut stmts = sql_parser.parse_statements()?;

    if stmts.len() != 1 {
        return Err(ParserError::ParserError(format!(
            "Cypher props translation produced {} SQL statements, expected 1",
            stmts.len()
        )));
    }

    Ok(stmts.remove(0))
}
