//! How many rows a statement returns, as far as ClickHouse's own query tree says (`EXPLAIN QUERY
//! TREE`, a line a node, two spaces a level). ClickHouse enforces no unique key, a primary key
//! being a sort key, so no `WHERE id = …` bounds a result. Two things do:
//!
//! - `LIMIT 1`, written as a number, which the tree gives as a constant: at most one row. `WITH
//!   TIES` lifts the bound, and a set operation's root is no query.
//! - An aggregate with no `GROUP BY`: one row, whatever it reads, an empty input included. The
//!   tree is what tells it from an aggregate grouped by nothing (`GROUP BY ()`) or by a constant,
//!   which returns no row over an empty input, and from a view that aggregates, which the tree
//!   leaves a table. An `arrayJoin` after the aggregation makes several rows of the one, and so
//!   does a `WITH FILL`, so neither is bounded. A `HAVING`, a `QUALIFY`, an `OFFSET`, a `LIMIT`
//!   or a `LIMIT BY` may drop the row, and a `SETTINGS` clause may turn it off
//!   (`empty_result_for_aggregation_by_empty_set`): at most one row.
//!
//! The tree's text is the server's to change, so what is not recognized proves nothing.

/// What a query tree proves of the statement's row count.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rows {
    /// The statement's own `LIMIT` is the constant 1 or 0, with no ties, or the statement is an
    /// aggregate with no `GROUP BY` whose row nothing multiplies.
    pub at_most_one: bool,
    /// The statement is one aggregate with no `GROUP BY`, and nothing after it drops or adds a row.
    pub one: bool,
    /// The statement is an aggregate with no `GROUP BY` whose row nothing multiplies: what bounds it.
    pub aggregate: bool,
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

/// The top query's sections: each one's heading, and the lines under it.
fn sections<'a>(lines: &[&'a str]) -> Vec<(&'a str, Vec<&'a str>)> {
    let mut out: Vec<(&str, Vec<&str>)> = Vec::new();
    for line in &lines[1..] {
        match indent(line) {
            2 => out.push((line.trim(), Vec::new())),
            n if n > 2 => {
                if let Some(last) = out.last_mut() {
                    last.1.push(*line);
                }
            }
            _ => {}
        }
    }
    out
}

/// The lines of a section that are the top query's own: those of a subquery inside it left out.
fn own<'a>(body: &[&'a str]) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut inside: Option<usize> = None;
    for line in body {
        if let Some(depth) = inside {
            if indent(line) > depth {
                continue;
            }
            inside = None;
        }
        let node = line.trim_start();
        if node.starts_with("QUERY id:") || node.starts_with("UNION id:") {
            inside = Some(indent(line));
            continue;
        }
        out.push(*line);
    }
    out
}

/// The whole number a `CONSTANT` node holds, as `constant_value: UInt64_1` writes it.
fn constant(line: &str) -> Option<u64> {
    let node = line.trim_start();
    if !node.starts_with("CONSTANT ") {
        return None;
    }
    let value = node.split("constant_value: ").nth(1)?.split(',').next()?;
    let (ty, digits) = value.rsplit_once('_')?;
    (ty.starts_with("UInt") || ty.starts_with("Int")).then(|| digits.parse().ok()).flatten()
}

/// What the tree's lines prove. `limit_given` says the statement writes a parameter as its LIMIT,
/// whose value the tree shows as the analysis's stand-in, not as a number the statement fixes.
pub fn rows(lines: &[&str], limit_given: bool) -> Rows {
    let Some(root) = lines.first().filter(|l| l.starts_with("QUERY id: ")) else { return Rows::default() };
    let sections = sections(lines);
    let body = |name: &str| sections.iter().find(|(heading, _)| *heading == name).map(|(_, body)| body.as_slice());
    let has = |name: &str| sections.iter().any(|(heading, _)| *heading == name);
    let ties = root.contains("is_limit_with_ties: 1");
    // The LIMIT's first line is its value; under it, the expression it was folded from.
    let limit = match body("LIMIT") {
        None => Some(None),
        Some(body) => body.first().and_then(|line| constant(line)).filter(|_| !limit_given).map(Some),
    };
    let settings = lines.iter().any(|l| l.trim_start().starts_with("SETTINGS"));
    let grouped = root.contains("is_group_by") || root.contains("group_by_type") || has("GROUP BY");
    let dropped = ["HAVING", "QUALIFY", "OFFSET", "LIMIT BY", "LIMIT BY LIMIT", "WINDOW"].iter().any(|s| has(s));
    let filled = lines.iter().any(|l| l.contains("with_fill: 1"));
    let projection = body("PROJECTION").map(own).unwrap_or_default();
    let aggregated = projection.iter().any(|l| l.contains("function_type: aggregate"));
    // An arrayJoin in what the statement computes after it aggregates; one in what it reads, its
    // tables and its WHERE, multiplies the rows the aggregate takes in.
    let multiplied = sections.iter().filter(|(heading, _)| !matches!(*heading, "JOIN TREE" | "WHERE" | "PREWHERE")).any(|(_, body)| own(body).iter().any(|l| l.contains("function_name: arrayJoin,")));
    let aggregate = aggregated && !multiplied && !grouped && !filled;
    let at_most_one = aggregate || (!ties && matches!(limit, Some(Some(0 | 1))));
    let kept = matches!(limit, Some(None)) || matches!(limit, Some(Some(n)) if n >= 1);
    let one = aggregate && !dropped && !settings && !ties && kept;
    Rows { at_most_one, one, aggregate }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(tree: &str, limit_given: bool) -> Rows {
        let lines: Vec<&str> = tree.lines().collect();
        rows(&lines, limit_given)
    }

    const AGGREGATE: &str = "QUERY id: 0\n  PROJECTION COLUMNS\n    n UInt64\n  PROJECTION\n    LIST id: 1, nodes: 1\n      FUNCTION id: 2, function_name: count, function_type: aggregate, result_type: UInt64\n  JOIN TREE\n    TABLE id: 3, alias: __table1, table_name: app.users";

    #[test]
    fn an_aggregate_with_no_group_by_is_one_row() {
        assert_eq!(read(AGGREGATE, false), Rows { at_most_one: true, one: true, aggregate: true });
        let limited = format!("{AGGREGATE}\n  LIMIT\n    CONSTANT id: 4, constant_value: UInt64_10, constant_value_type: UInt64");
        assert_eq!(read(&limited, false), Rows { at_most_one: true, one: true, aggregate: true });
        let one = format!("{AGGREGATE}\n  LIMIT\n    CONSTANT id: 4, constant_value: UInt64_1, constant_value_type: UInt64");
        assert_eq!(read(&one, false), Rows { at_most_one: true, one: true, aggregate: true });
    }

    #[test]
    fn what_may_drop_or_add_a_row_leaves_it_unproven() {
        // What may drop the aggregate's row leaves at most one, and what may add rows no bound.
        for (after, bounded) in [
            ("  GROUP BY\n    LIST id: 4, nodes: 1\n      CONSTANT id: 5, constant_value: Tuple_(), constant_value_type: Tuple()", false),
            ("  HAVING\n    FUNCTION id: 6, function_name: greater, function_type: ordinary, result_type: UInt8", true),
            ("  OFFSET\n    CONSTANT id: 13, constant_value: UInt64_0, constant_value_type: UInt64", true),
            ("  LIMIT\n    CONSTANT id: 4, constant_value: UInt64_0, constant_value_type: UInt64", true),
            ("  SETTINGS empty_result_for_aggregation_by_empty_set=1", true),
            ("  ORDER BY\n    LIST id: 6, nodes: 1\n      SORT id: 7, sort_direction: ASCENDING, with_fill: 1", false),
            ("  ORDER BY\n    LIST id: 6, nodes: 1\n      SORT id: 7, sort_direction: ASCENDING, with_fill: 0\n        FUNCTION id: 8, function_name: arrayJoin, function_type: ordinary, result_type: UInt8", false),
            ("  QUALIFY\n    FUNCTION id: 4, function_name: equals, function_type: ordinary, result_type: UInt8", true),
        ] {
            let found = read(&format!("{AGGREGATE}\n{after}"), false);
            assert_eq!((found.one, found.at_most_one), (false, bounded), "{after}");
        }
        assert_eq!(read(&AGGREGATE.replace("QUERY id: 0", "QUERY id: 0, is_group_by_with_totals: 1"), false), Rows::default());
        // An arrayJoin in what the aggregate reads multiplies its input, not its row.
        let read_from = format!("{AGGREGATE}\n  WHERE\n    FUNCTION id: 9, function_name: arrayJoin, function_type: ordinary, result_type: UInt8");
        assert!(read(&read_from, false).one);
        // An arrayJoin in the list makes several rows of the one, and an aggregate inside a
        // subquery is the subquery's.
        let joined = AGGREGATE.replace("      FUNCTION id: 2, function_name: count, function_type: aggregate, result_type: UInt64", "      FUNCTION id: 2, function_name: count, function_type: aggregate, result_type: UInt64\n      FUNCTION id: 7, function_name: arrayJoin, function_type: ordinary, result_type: UInt8");
        assert_eq!(read(&joined, false), Rows::default());
        let inner = "QUERY id: 0\n  PROJECTION\n    LIST id: 1, nodes: 1\n      CONSTANT id: 2, constant_value: UInt64_5, constant_value_type: Nullable(UInt64)\n        EXPRESSION\n          QUERY id: 3, is_subquery: 1\n            PROJECTION\n              LIST id: 4, nodes: 1\n                FUNCTION id: 5, function_name: count, function_type: aggregate, result_type: UInt64\n  JOIN TREE\n    TABLE id: 10, table_name: app.users";
        assert_eq!(read(inner, false), Rows::default());
        // A view that aggregates is a table to the statement that reads it.
        assert_eq!(read("QUERY id: 0\n  PROJECTION\n    LIST id: 1, nodes: 1\n      COLUMN id: 2, column_name: n, result_type: UInt64, source_id: 3\n  JOIN TREE\n    TABLE id: 3, alias: __table1, table_name: app.counted", false), Rows::default());
    }

    #[test]
    fn a_limit_of_one_is_at_most_one_row() {
        let limited = "QUERY id: 0\n  PROJECTION\n    LIST id: 1, nodes: 1\n      COLUMN id: 2, column_name: id, result_type: UInt64, source_id: 3\n  JOIN TREE\n    TABLE id: 3, table_name: app.users\n  LIMIT\n    CONSTANT id: 4, constant_value: UInt64_1, constant_value_type: UInt64";
        assert_eq!(read(limited, false), Rows { at_most_one: true, one: false, aggregate: false });
        // A parameter as the limit is whatever the caller passes.
        assert_eq!(read(limited, true), Rows::default());
        assert_eq!(read(&limited.replace("QUERY id: 0", "QUERY id: 0, is_limit_with_ties: 1"), false), Rows::default());
        assert_eq!(read(&limited.replace("UInt64_1,", "UInt64_2,"), false), Rows::default());
        // A set operation's root is no query, and neither is text the engine does not know.
        assert_eq!(read("UNION id: 0, union_mode: UNION_ALL\n  QUERIES\n    LIST id: 1, nodes: 2", false), Rows::default());
        assert_eq!(read("", false), Rows::default());
    }
}
