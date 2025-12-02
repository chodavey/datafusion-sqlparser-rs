use sqlparser::dialect::CypherDialect;
use sqlparser::parser::Parser;

#[test]
fn cypher_create_labeled_node_with_properties() {
    let dialect = CypherDialect::default();
    let mut parser = Parser::new(&dialect)
        .try_with_sql("CREATE (a:Person {name: 'Jorge', age: 26});")
        .unwrap();

    let mut stmts = parser.parse_statements().unwrap();
    assert_eq!(stmts.len(), 1, "expected exactly one statement");

    let sql = stmts.remove(0).to_string();

    assert_eq!(sql, "INSERT INTO nodes (label, properties) VALUES ('Person', '{\"name\":\"Jorge\",\"age\":26}')");
}

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
fn cypher_match_count_unlabeled_default_alias() {
    let dialect = CypherDialect::default();
    let mut parser = Parser::new(&dialect)
        .try_with_sql("MATCH (p) RETURN count(p);")
        .unwrap();

    let mut stmts = parser.parse_statements().unwrap();
    assert_eq!(stmts.len(), 1);
    let sql = stmts.remove(0).to_string();

    // default alias should be "count"
    assert_eq!(sql, "SELECT COUNT(*) AS count FROM nodes AS p");
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
fn cypher_match_count_labeled_default_alias() {
    let dialect = CypherDialect::default();
    let mut parser = Parser::new(&dialect)
        .try_with_sql("MATCH (p:Person) RETURN count(p);")
        .unwrap();

    let mut stmts = parser.parse_statements().unwrap();
    assert_eq!(stmts.len(), 1);
    let sql = stmts.remove(0).to_string();

    assert_eq!(
        sql,
        "SELECT COUNT(*) AS count FROM nodes AS p WHERE p.label = 'Person'"
    );
}

#[test]
fn cypher_match_count_unlabeled_limit_default_alias() {
    let dialect = CypherDialect::default();
    let mut parser = Parser::new(&dialect)
        .try_with_sql("MATCH (p) RETURN count(p) LIMIT 5;")
        .unwrap();

    let mut stmts = parser.parse_statements().unwrap();
    assert_eq!(stmts.len(), 1);
    let sql = stmts.remove(0).to_string();

    assert_eq!(sql, "SELECT COUNT(*) AS count FROM nodes AS p LIMIT 5");
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
        "SELECT json_extract(p.properties, '$.name') AS name, \
        json_extract(p.properties, '$.age') AS age \
        FROM nodes AS p WHERE p.label = 'Person' LIMIT 10"
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
        "SELECT p.label AS labels, \
        json_extract(p.properties, '$.name') AS name, \
        json_extract(p.properties, '$.age') AS age \
        FROM nodes AS p LIMIT 10"
    );
}

#[test]
fn cypher_match_return_node_unlabeled() {
    let dialect = CypherDialect::default();
    let mut parser = Parser::new(&dialect)
        .try_with_sql("MATCH (p) RETURN p;")
        .unwrap();

    let mut stmts = parser.parse_statements().unwrap();
    assert_eq!(stmts.len(), 1);
    let sql = stmts.remove(0).to_string();

    // Should select a JSON node column
    assert!(sql.starts_with("SELECT"));
    assert!(sql.contains("json_object"));
    assert!(sql.contains("FROM nodes AS p"));
}

#[test]
fn cypher_match_return_node_labeled() {
    let dialect = CypherDialect::default();
    let mut parser = Parser::new(&dialect)
        .try_with_sql("MATCH (p:Person) RETURN p AS person;")
        .unwrap();

    let mut stmts = parser.parse_statements().unwrap();
    assert_eq!(stmts.len(), 1);
    let sql = stmts.remove(0).to_string();

    // Should select a JSON node column aliased as `person` and filter by label
    assert!(sql.starts_with("SELECT"));
    assert!(sql.contains("json_object"));
    assert!(sql.contains("AS person"));
    assert!(sql.contains("FROM nodes AS p WHERE p.label = 'Person'"));
}

#[test]
fn cypher_match_count_mismatched_var_errors() {
    let dialect = CypherDialect::default();
    let mut parser = Parser::new(&dialect)
        .try_with_sql("MATCH (p) RETURN count(q);")
        .unwrap();

    let res = parser.parse_statements();
    assert!(res.is_err());
    let err = res.unwrap_err().to_string();
    assert!(
        err.contains("COUNT() variable must match MATCH() variable"),
        "unexpected error: {err}"
    );
}

#[test]
fn cypher_match_labels_mismatched_var_errors() {
    let dialect = CypherDialect::default();
    let mut parser = Parser::new(&dialect)
        .try_with_sql("MATCH (p) RETURN labels(q);")
        .unwrap();

    let res = parser.parse_statements();
    assert!(res.is_err());
    let err = res.unwrap_err().to_string();
    assert!(
        err.contains("labels() variable must match MATCH() variable"),
        "unexpected error: {err}"
    );
}

#[test]
fn cypher_match_delete_node_by_property() {
    let dialect = CypherDialect::default();
    let mut parser = Parser::new(&dialect)
        .try_with_sql("MATCH (n:Person {name: 'David'}) DELETE n;")
        .unwrap();

    let mut stmts = parser.parse_statements().unwrap();
    assert_eq!(stmts.len(), 1, "expected exactly one statement");

    let sql = stmts.remove(0).to_string();

    // Basic shape: DELETE FROM nodes
    assert!(
        sql.to_uppercase()
            .starts_with("DELETE FROM nodes".to_uppercase().as_str()),
        "unexpected SQL generated for MATCH ... DELETE: {sql}"
    );

    // Ensure label filtering is preserved
    assert!(
        sql.contains("label = 'Person'") || sql.contains("label = \"Person\""),
        "expected label filter in generated SQL: {sql}"
    );

    // Ensure property filter on name is present
    assert!(
        sql.contains("$.name") || sql.to_lowercase().contains("name") && sql.contains("David"),
        "expected property filter on name='David' in generated SQL: {sql}"
    );
}
