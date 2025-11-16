use sqlparser::dialect::CypherDialect;
use sqlparser::parser::Parser;

#[test]
fn cypher_match_count_unlabeled() {
    let dialect = CypherDialect::default();
    let mut parser = Parser::new(&dialect)
        .try_with_sql("MATCH (p) RETURN count(p) AS nodes;")
        .unwrap();

    let mut stmts = parser.parse_statements().unwrap();
    assert_eq!(stmts.len(), 1);
    let sql = stmts.remove(0).to_string();

    assert_eq!(sql, "SELECT COUNT(*) AS nodes FROM nodes AS p");
}

#[test]
fn cypher_match_count_labeled() {
    let dialect = CypherDialect::default();
    let mut parser = Parser::new(&dialect)
        .try_with_sql("MATCH (p:Person) RETURN count(p) AS persons;")
        .unwrap();

    let mut stmts = parser.parse_statements().unwrap();
    assert_eq!(stmts.len(), 1);
    let sql = stmts.remove(0).to_string();

    assert_eq!(
        sql,
        "SELECT COUNT(*) AS persons FROM nodes AS p WHERE p.label = 'Person'"
    );
}

#[test]
fn cypher_match_props_labeled_limit() {
    let dialect = CypherDialect::default();
    let mut parser = Parser::new(&dialect)
        .try_with_sql("MATCH (p:Person) RETURN p.name AS name, p.age AS age LIMIT 10;")
        .unwrap();

    let mut stmts = parser.parse_statements().unwrap();
    assert_eq!(stmts.len(), 1);
    let sql = stmts.remove(0).to_string();

    assert_eq!(
        sql,
        "SELECT p.name AS name, p.age AS age FROM nodes AS p WHERE p.label = 'Person' LIMIT 10"
    );
}

#[test]
fn cypher_match_labels_and_props_unlabeled_limit() {
    let dialect = CypherDialect::default();
    let mut parser = Parser::new(&dialect)
        .try_with_sql(
            "MATCH (p) RETURN labels(p) AS labels, p.name AS name, p.age AS age LIMIT 10;",
        )
        .unwrap();

    let mut stmts = parser.parse_statements().unwrap();
    assert_eq!(stmts.len(), 1);
    let sql = stmts.remove(0).to_string();

    assert_eq!(
        sql,
        "SELECT p.label AS labels, p.name AS name, p.age AS age FROM nodes AS p LIMIT 10"
    );
}
