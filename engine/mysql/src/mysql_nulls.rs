//! What a SELECT's WHERE proves of its result columns, which the server's NOT NULL flag does not
//! say: measured on 8.4, `select p.title from users u left join posts p on p.user_id = u.id where
//! p.published_at is not null` reports `title` nullable, though no row with `p` null-extended
//! passes the test. Postgres and SQLite read the same rule off the statement
//! (`engine/postgres/src/infer.rs`, `strict_in`).
//!
//! A test at the top of the WHERE that is never true where a column is NULL is strict in it
//! ([`strict_in`]). Two things follow, and the engine reads both:
//!
//! - A row the SELECT returns passed the test, so a result column that is that column, as written,
//!   is not NULL ([`tested`]).
//! - A row an outer join null-extends holds NULL in every column of the side it nulls, so a test
//!   strict in a column of that side rejects every such row: the join returns what an inner join
//!   returns, which is the outer-join simplification the server's own optimizer does. The engine
//!   writes the statement with that join inner ([`reduce`]) and prepares it, and the server's flags
//!   for it hold for the statement as written.

use sqlparser::ast::{self, BinaryOperator, Expr, GroupByExpr, GroupByWithModifier, JoinConstraint, JoinOperator, Query, SelectItem, SetExpr, TableFactor, TableWithJoins, UnaryOperator};

/// Which arguments of a built-in function make it NULL where they are NULL.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Strict {
    /// Every argument, however many it takes.
    Every,
    /// These positions, from 0; NULL elsewhere may give a value.
    At(&'static [usize]),
}

use Strict::{At, Every};

/// MySQL's built-in functions that are NULL wherever one of the arguments named is NULL, each
/// measured on 8.4.11: the call with that argument NULL and the others values it is otherwise not
/// NULL for, which it then was. `concat` is NULL for a NULL anywhere, while `concat_ws` skips one
/// after its separator, `nullif(a, NULL)` is `a`, `json_set` takes a NULL value as JSON's null and
/// `week` a NULL mode as the default one; a position not named, and a function not here, may
/// give a value for NULL, as `coalesce`, `ifnull`, `if`, `char`, `field`, `interval`,
/// `json_array`, `json_object`, `quote` and `isnull` do. That a function is NULL for more than its
/// NULL arguments (`str_to_date` for text that is no date) changes nothing here.
const STRICT_FUNCTIONS: &[(&str, Strict)] = &[
    // Strings.
    ("ascii", Every), ("bin", Every), ("bit_length", Every), ("char_length", Every), ("character_length", Every), ("compress", Every), ("concat", Every), ("concat_ws", At(&[0])), ("conv", Every), ("crc32", Every), ("elt", At(&[0])), ("export_set", Every), ("find_in_set", Every), ("format", At(&[0, 1])), ("from_base64", Every), ("hex", Every), ("insert", Every), ("instr", Every), ("lcase", Every), ("left", Every), ("length", Every), ("locate", Every), ("lower", Every), ("lpad", Every), ("ltrim", Every), ("make_set", At(&[0])), ("md5", Every), ("mid", Every), ("oct", Every), ("octet_length", Every), ("ord", Every), ("regexp_instr", At(&[0, 1])), ("regexp_like", At(&[0, 1])), ("regexp_replace", At(&[0, 1, 2])), ("regexp_substr", At(&[0, 1])), ("repeat", Every), ("replace", Every), ("reverse", Every), ("right", Every), ("rpad", Every), ("rtrim", Every), ("sha", Every), ("sha1", Every), ("sha2", Every), ("soundex", Every), ("space", Every), ("strcmp", Every), ("substr", Every), ("substring", Every), ("substring_index", Every), ("to_base64", Every), ("ucase", Every), ("uncompress", Every), ("uncompressed_length", Every), ("unhex", Every), ("upper", Every), ("weight_string", Every),
    // Numbers.
    ("abs", Every), ("acos", Every), ("asin", Every), ("atan", Every), ("atan2", Every), ("bit_count", Every), ("ceil", Every), ("ceiling", Every), ("cos", Every), ("cot", Every), ("degrees", Every), ("exp", Every), ("floor", Every), ("ln", Every), ("log", Every), ("log10", Every), ("log2", Every), ("mod", Every), ("pow", Every), ("power", Every), ("radians", Every), ("round", Every), ("sign", Every), ("sin", Every), ("sqrt", Every), ("tan", Every), ("truncate", Every),
    // Dates and times; `timestampadd` and `timestampdiff` take a unit first, which is no value.
    ("adddate", Every), ("addtime", Every), ("convert_tz", Every), ("date", Every), ("date_add", Every), ("date_format", Every), ("date_sub", Every), ("datediff", Every), ("day", Every), ("dayname", Every), ("dayofmonth", Every), ("dayofweek", Every), ("dayofyear", Every), ("from_days", Every), ("from_unixtime", Every), ("hour", Every), ("last_day", Every), ("makedate", Every), ("maketime", Every), ("microsecond", Every), ("minute", Every), ("month", Every), ("monthname", Every), ("period_add", Every), ("period_diff", Every), ("quarter", Every), ("sec_to_time", Every), ("second", Every), ("str_to_date", Every), ("subdate", Every), ("subtime", Every), ("time", Every), ("time_format", Every), ("time_to_sec", Every), ("timediff", Every), ("timestamp", Every), ("timestampadd", At(&[1, 2])), ("timestampdiff", At(&[1, 2])), ("to_days", Every), ("to_seconds", Every), ("unix_timestamp", Every), ("week", At(&[0])), ("weekday", Every), ("weekofyear", Every), ("year", Every), ("yearweek", At(&[0])),
    // JSON: a document's editors are NULL for a NULL document or first path, and not for a NULL
    // value, which they write as JSON's null.
    ("json_array_append", At(&[0, 1])), ("json_array_insert", At(&[0, 1])), ("json_contains", Every), ("json_contains_path", Every), ("json_depth", Every), ("json_extract", Every), ("json_insert", At(&[0, 1])), ("json_keys", Every), ("json_length", Every), ("json_merge_patch", At(&[1])), ("json_merge_preserve", Every), ("json_overlaps", Every), ("json_pretty", Every), ("json_quote", Every), ("json_remove", Every), ("json_replace", At(&[0, 1])), ("json_search", At(&[0, 1, 2, 4])), ("json_set", At(&[0, 1])), ("json_storage_size", Every), ("json_type", Every), ("json_unquote", Every), ("json_valid", Every), ("json_value", At(&[0])),
    // The rest.
    ("any_value", Every), ("bin_to_uuid", At(&[0])), ("format_bytes", Every), ("inet6_aton", Every), ("inet6_ntoa", Every), ("inet_aton", Every), ("inet_ntoa", Every), ("is_ipv4", Every), ("is_ipv6", Every), ("st_astext", At(&[0])), ("st_geomfromtext", At(&[0])), ("st_x", At(&[0])), ("st_y", At(&[0])), ("statement_digest_text", Every), ("uuid_to_bin", At(&[0])),
];

/// Which arguments of a call make it NULL where they are NULL, by [`STRICT_FUNCTIONS`].
fn strict_args(name: &str) -> Option<Strict> {
    STRICT_FUNCTIONS.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
}

/// The statement with every outer join inner whose null-extended rows a strict test rejects: a
/// LEFT JOIN whose right side holds a column the test is strict in, and a RIGHT JOIN whose left
/// side does. The tests are the WHERE's, and the ON clause of each inner join, which rejects such
/// rows of the joins before it (`posts p left join users u on … join teams t on t.id = u.team_id`).
/// Only a column qualified with the name of a FROM item counts, and only in a plain SELECT. `None`
/// when no join changes.
pub fn reduce(q: &Query) -> Option<Query> {
    let SetExpr::Select(select) = q.body.as_ref() else { return None };
    let quals = select.selection.as_ref().map(qualifiers).unwrap_or_default();
    let mut from = select.from.clone();
    let mut changed = false;
    for item in &mut from {
        changed |= reduce_joins(item, &quals);
    }
    if !changed {
        return None;
    }
    let mut reduced = q.clone();
    if let SetExpr::Select(select) = reduced.body.as_mut() {
        select.from = from;
    }
    Some(reduced)
}

/// The FROM items a test is strict in a column of, by the name that qualifies it.
fn qualifiers(e: &Expr) -> Vec<String> {
    strict_in(e)
        .into_iter()
        .filter_map(|c| match c.as_slice() {
            [q, _] => Some(q.clone()),
            _ => None,
        })
        .collect()
}

/// Make inner each outer join of `item` whose null-extended side holds a FROM item named in
/// `quals` or in the ON clause of an inner join after it, inside a nested join too; whether any
/// changed. The joins are read from the last, so a join made inner lends its ON clause to the
/// ones before it.
fn reduce_joins(item: &mut TableWithJoins, quals: &[String]) -> bool {
    let mut quals = quals.to_vec();
    let mut changed = false;
    for i in (0..item.joins.len()).rev() {
        let nulled = match &item.joins[i].join_operator {
            JoinOperator::Left(_) | JoinOperator::LeftOuter(_) => names(&item.joins[i].relation),
            JoinOperator::Right(_) | JoinOperator::RightOuter(_) => {
                let mut left = names(&item.relation);
                for join in &item.joins[..i] {
                    left.extend(names(&join.relation));
                }
                left
            }
            _ => Vec::new(),
        };
        if nulled.iter().any(|n| quals.contains(n)) {
            if let JoinOperator::Left(c) | JoinOperator::LeftOuter(c) | JoinOperator::Right(c) | JoinOperator::RightOuter(c) = &item.joins[i].join_operator {
                item.joins[i].join_operator = JoinOperator::Inner(c.clone());
                changed = true;
            }
        }
        if let JoinOperator::Join(JoinConstraint::On(on)) | JoinOperator::Inner(JoinConstraint::On(on)) | JoinOperator::StraightJoin(JoinConstraint::On(on)) = &item.joins[i].join_operator {
            quals.extend(qualifiers(on));
        }
        changed |= nested(&mut item.joins[i].relation, &quals);
    }
    changed | nested(&mut item.relation, &quals)
}

fn nested(factor: &mut TableFactor, qualifiers: &[String]) -> bool {
    match factor {
        TableFactor::NestedJoin { table_with_joins, .. } => reduce_joins(table_with_joins, qualifiers),
        _ => false,
    }
}

/// The names a FROM item's columns are qualified with, in lower case, through its nested joins.
fn names(factor: &TableFactor) -> Vec<String> {
    match factor {
        TableFactor::NestedJoin { table_with_joins, .. } => {
            let mut out = names(&table_with_joins.relation);
            for join in &table_with_joins.joins {
                out.extend(names(&join.relation));
            }
            out
        }
        TableFactor::Table { name, alias, .. } => match alias {
            Some(a) => vec![a.name.value.to_ascii_lowercase()],
            None => match name.0.last() {
                Some(ast::ObjectNamePart::Identifier(i)) => vec![i.value.to_ascii_lowercase()],
                _ => Vec::new(),
            },
        },
        TableFactor::Derived { alias, .. } | TableFactor::TableFunction { alias, .. } | TableFactor::Function { alias, .. } | TableFactor::JsonTable { alias, .. } => alias.iter().map(|a| a.name.value.to_ascii_lowercase()).collect(),
        _ => Vec::new(),
    }
}

/// The projection positions of a plain SELECT whose item is a bare column the WHERE is strict in:
/// such a column is not NULL in a row that passed. Nothing where a wildcard makes the positions
/// unreadable, or where a ROLLUP adds rows with NULL in the grouped columns.
pub fn tested(q: &Query) -> Vec<usize> {
    let SetExpr::Select(select) = q.body.as_ref() else { return Vec::new() };
    let Some(selection) = select.selection.as_ref() else { return Vec::new() };
    if select.projection.iter().any(|i| matches!(i, SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(..))) || rolls_up(&select.group_by) {
        return Vec::new();
    }
    let strict = strict_in(selection);
    if strict.is_empty() {
        return Vec::new();
    }
    let mut from_names = Vec::new();
    let (mut using, mut natural) = (Vec::new(), false);
    for item in &select.from {
        from_names.extend(names(&item.relation));
        for join in &item.joins {
            from_names.extend(names(&join.relation));
        }
        merged(item, &mut using, &mut natural);
    }
    let same = |tested: &[String], read: &[String]| -> bool {
        if tested == read {
            return true;
        }
        // A bare name resolves to exactly one FROM item's column, so that is the one, except a
        // name a USING or NATURAL join merges: the merged column is one side's under a LEFT or
        // RIGHT join, so a test of it proves nothing of the other's own. A test of a side's
        // column proves the merged one, which is NULL only where both sides are.
        let (bare, qualified) = match (tested.len(), read.len()) {
            (1, 2) if natural || using.contains(&tested[0]) => return false,
            (1, 2) => (tested, read),
            (2, 1) => (read, tested),
            _ => return false,
        };
        bare[0] == qualified[1] && from_names.contains(&qualified[0])
    };
    select
        .projection
        .iter()
        .enumerate()
        .filter(|(_, item)| match item {
            SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } => column(e).is_some_and(|read| strict.iter().any(|t| same(t, &read))),
            _ => false,
        })
        .map(|(i, _)| i)
        .collect()
}

/// The column names USING joins merge, in lower case, and whether a NATURAL join merges any.
fn merged(item: &TableWithJoins, using: &mut Vec<String>, natural: &mut bool) {
    let factor = |f: &TableFactor, using: &mut Vec<String>, natural: &mut bool| {
        if let TableFactor::NestedJoin { table_with_joins, .. } = f {
            merged(table_with_joins, using, natural);
        }
    };
    factor(&item.relation, using, natural);
    for join in &item.joins {
        factor(&join.relation, using, natural);
        let constraint = match &join.join_operator {
            JoinOperator::Join(c) | JoinOperator::Inner(c) | JoinOperator::Left(c) | JoinOperator::LeftOuter(c) | JoinOperator::Right(c) | JoinOperator::RightOuter(c) | JoinOperator::FullOuter(c) | JoinOperator::CrossJoin(c) | JoinOperator::StraightJoin(c) => Some(c),
            _ => None,
        };
        match constraint {
            Some(JoinConstraint::Natural) => *natural = true,
            Some(JoinConstraint::Using(names)) => using.extend(names.iter().filter_map(|n| match n.0.last() {
                Some(ast::ObjectNamePart::Identifier(i)) => Some(i.value.to_ascii_lowercase()),
                _ => None,
            })),
            _ => {}
        }
    }
}

/// Whether a GROUP BY adds super-aggregate rows: WITH ROLLUP, or ROLLUP, CUBE or GROUPING SETS.
/// Such a row aggregates every row, and is there over none.
pub(crate) fn rolls_up(group_by: &GroupByExpr) -> bool {
    let (exprs, modifiers): (&[Expr], &[GroupByWithModifier]) = match group_by {
        GroupByExpr::All(modifiers) => (&[], modifiers),
        GroupByExpr::Expressions(exprs, modifiers) => (exprs, modifiers),
    };
    !modifiers.is_empty() || exprs.iter().any(|e| matches!(e, Expr::Rollup(_) | Expr::Cube(_) | Expr::GroupingSets(_)))
}

/// A bare column reference, as its parts in lower case.
fn column(e: &Expr) -> Option<Vec<String>> {
    match e {
        Expr::Nested(inner) => column(inner),
        Expr::Identifier(i) => Some(vec![i.value.to_ascii_lowercase()]),
        Expr::CompoundIdentifier(parts) if parts.len() == 2 => Some(parts.iter().map(|p| p.value.to_ascii_lowercase()).collect()),
        _ => None,
    }
}

/// The columns a predicate at the top of a WHERE is strict in: a row with NULL in one of them does
/// not pass. Under AND, either side's; under OR, the ones every arm is strict in; `IS NOT NULL`,
/// `IS TRUE` and `IS FALSE`; NOT over `IS NULL`; `IN` over a subquery (not `NOT IN`, which an empty
/// subquery makes true for NULL); and every column the test is NULL for ([`nulls_with`]), a bare
/// value taken as a boolean among them.
pub fn strict_in(e: &Expr) -> Vec<Vec<String>> {
    match e {
        Expr::Nested(inner) => strict_in(inner),
        Expr::BinaryOp { left, op: BinaryOperator::And, right } => {
            let mut out = strict_in(left);
            for c in strict_in(right) {
                if !out.contains(&c) {
                    out.push(c);
                }
            }
            out
        }
        Expr::BinaryOp { left, op: BinaryOperator::Or, right } => {
            let right = strict_in(right);
            strict_in(left).into_iter().filter(|c| right.contains(c)).collect()
        }
        Expr::UnaryOp { op: UnaryOperator::Not, expr } => match strip(expr) {
            Expr::IsNull(inner) => nulls_with(inner),
            other => nulls_with(other),
        },
        Expr::IsNotNull(inner) | Expr::IsTrue(inner) | Expr::IsFalse(inner) => nulls_with(inner),
        Expr::InSubquery { expr, negated: false, .. } => nulls_with(expr),
        other => nulls_with(other),
    }
}

fn strip(e: &Expr) -> &Expr {
    match e {
        Expr::Nested(inner) => strip(inner),
        other => other,
    }
}

/// The columns an expression is NULL for, wherever they are NULL: the column itself, and through
/// an operator or a function, every column an operand that makes it NULL is NULL for. Each was
/// measured on 8.4.11 as the functions of [`STRICT_FUNCTIONS`] were: a comparison but `<=>`,
/// arithmetic, a bitwise operator, XOR, NOT, a sign, `->` and `->>` in their document,
/// `[NOT] LIKE`, `[NOT] REGEXP`, `MEMBER OF`, a cast, CONVERT, COLLATE, an INTERVAL's value, and
/// the forms the parser reads apart: `SUBSTRING`, `TRIM`, `POSITION`, `EXTRACT`, `CEIL` and
/// `FLOOR`. `[NOT] IN` over a list and `[NOT] BETWEEN` are NULL for a NULL value tested, and not
/// for a NULL in the list or a bound: `1 in (1, NULL)` is 1, and `5 between NULL and 1` is 0. AND
/// and OR give a value for NULL (`NULL and 0` is 0), and so do `IS`, `<=>`, CASE and a subquery.
pub(crate) fn nulls_with(e: &Expr) -> Vec<Vec<String>> {
    let all = |exprs: &[&Expr]| -> Vec<Vec<String>> { exprs.iter().flat_map(|e| nulls_with(e)).collect() };
    match e {
        Expr::Nested(inner) => nulls_with(inner),
        Expr::Identifier(_) | Expr::CompoundIdentifier(_) => column(e).into_iter().collect(),
        Expr::BinaryOp {
            left,
            op:
                BinaryOperator::Eq
                | BinaryOperator::NotEq
                | BinaryOperator::Lt
                | BinaryOperator::Gt
                | BinaryOperator::LtEq
                | BinaryOperator::GtEq
                | BinaryOperator::Plus
                | BinaryOperator::Minus
                | BinaryOperator::Multiply
                | BinaryOperator::Divide
                | BinaryOperator::Modulo
                | BinaryOperator::MyIntegerDivide
                | BinaryOperator::BitwiseAnd
                | BinaryOperator::BitwiseOr
                | BinaryOperator::BitwiseXor
                | BinaryOperator::PGBitwiseShiftLeft
                | BinaryOperator::PGBitwiseShiftRight
                | BinaryOperator::Xor
                | BinaryOperator::Regexp,
            right,
        } => all(&[&**left, &**right]),
        Expr::BinaryOp { left, op: BinaryOperator::Arrow | BinaryOperator::LongArrow, .. } => nulls_with(left),
        Expr::UnaryOp { op: UnaryOperator::Not | UnaryOperator::Minus | UnaryOperator::Plus | UnaryOperator::BitwiseNot, expr } => nulls_with(expr),
        Expr::Like { expr, pattern, any: false, .. } | Expr::RLike { expr, pattern, .. } => all(&[&**expr, &**pattern]),
        Expr::InList { expr, .. } | Expr::Between { expr, .. } => nulls_with(expr),
        Expr::MemberOf(m) => all(&[&*m.value, &*m.array]),
        Expr::Cast { expr, .. } | Expr::Collate { expr, .. } | Expr::Convert { expr, .. } | Expr::Extract { expr, .. } | Expr::Ceil { expr, .. } | Expr::Floor { expr, .. } => nulls_with(expr),
        Expr::Interval(i) => nulls_with(&i.value),
        Expr::Substring { expr, substring_from, substring_for, .. } => {
            let mut out = nulls_with(expr);
            out.extend(substring_from.iter().chain(substring_for).flat_map(|x| nulls_with(x)));
            out
        }
        Expr::Trim { expr, trim_what, trim_characters: None, .. } => {
            let mut out = nulls_with(expr);
            out.extend(trim_what.iter().flat_map(|x| nulls_with(x)));
            out
        }
        Expr::Position { expr, r#in } => all(&[&**expr, &**r#in]),
        Expr::Function(f) if f.over.is_none() && f.filter.is_none() && f.within_group.is_empty() => {
            let Some(strict) = strict_args(&crate::mysql::builtin(f)) else { return Vec::new() };
            match &f.args {
                ast::FunctionArguments::List(list) if list.duplicate_treatment.is_none() && list.clauses.is_empty() => list
                    .args
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| match strict {
                        Every => true,
                        At(positions) => positions.contains(i),
                    })
                    .flat_map(|(_, a)| match a {
                        ast::FunctionArg::Unnamed(ast::FunctionArgExpr::Expr(x)) => nulls_with(x),
                        _ => Vec::new(),
                    })
                    .collect(),
                _ => Vec::new(),
            }
        }
        _ => Vec::new(),
    }
}

/// The aggregates that, as window functions, are NULL only where no non-null value reaches the
/// frame: over a frame that holds the current row they are as NULL as their argument there.
const WINDOW_FOLLOWING: &[&str] = &["sum", "avg", "min", "max"];
/// The aggregates that are NULL only over no rows: over a frame that holds the current row they
/// are an array or an object, NULL inputs and all (a NULL key is an error).
const WINDOW_NEVER_NULL: &[&str] = &["json_arrayagg", "json_objectagg"];

/// What a window call says of its result column, where its frame always holds the current row.
#[derive(Debug, PartialEq)]
pub(crate) enum Windowed<'a> {
    /// As NULL as this argument on the current row.
    Follows(&'a Expr),
    /// Never NULL.
    Always,
}

/// What a select-list item of a plain SELECT says of its column when it is an aggregate called as
/// a window function whose frame always holds the current row ([`frame_holds_current_row`]):
/// `sum`, `avg`, `min` and `max` follow their argument, and `json_arrayagg` and `json_objectagg`
/// are never NULL. `None` for anything else, a call with DISTINCT or a FILTER among them.
pub(crate) fn windowed<'a>(e: &'a Expr, named: &[ast::NamedWindowDefinition]) -> Option<Windowed<'a>> {
    let e = strip(e);
    let Expr::Function(f) = e else { return None };
    let over = f.over.as_ref()?;
    if f.filter.is_some() || !f.within_group.is_empty() || f.null_treatment.is_some() {
        return None;
    }
    let ast::FunctionArguments::List(list) = &f.args else { return None };
    if list.duplicate_treatment.is_some() || !list.clauses.is_empty() {
        return None;
    }
    let args: Vec<&Expr> = list
        .args
        .iter()
        .map(|a| match a {
            ast::FunctionArg::Unnamed(ast::FunctionArgExpr::Expr(x)) => Some(x),
            _ => None,
        })
        .collect::<Option<_>>()?;
    let name = crate::mysql::builtin(f);
    if !frame_holds_current_row(over, named) {
        return None;
    }
    match (name.as_str(), args.as_slice()) {
        (n, [arg]) if WINDOW_FOLLOWING.contains(&n) => Some(Windowed::Follows(arg)),
        ("json_arrayagg", [_]) | ("json_objectagg", [_, _]) if WINDOW_NEVER_NULL.contains(&name.as_str()) => Some(Windowed::Always),
        _ => None,
    }
}

/// Whether the frame of an OVER clause always holds the current row, given the SELECT's WINDOW
/// clause. No frame clause is the whole partition, or with ORDER BY every row up to the current
/// row's last peer. A ROWS frame that starts UNBOUNDED PRECEDING, `n PRECEDING` or at CURRENT ROW
/// and ends at CURRENT ROW, `n FOLLOWING` or UNBOUNDED FOLLOWING holds it, and so does a RANGE
/// frame whose bounds are unbounded or CURRENT ROW. A start `n FOLLOWING` or an end `n PRECEDING`
/// can leave the row out, and so can a RANGE frame with an offset: measured on 8.4, over a
/// DATETIME that holds a zero date, `range between interval 1 day preceding and interval 1 day
/// following` finds no row for any row whose date is not NULL. A window named in the OVER clause,
/// alone or as the base of a definition, lends its frame where the definition has none of its own.
pub(crate) fn frame_holds_current_row(over: &ast::WindowType, named: &[ast::NamedWindowDefinition]) -> bool {
    let mut spec = match over {
        ast::WindowType::WindowSpec(spec) => spec,
        ast::WindowType::NamedWindow(name) => match lookup(name, named) {
            Some(spec) => spec,
            None => return false,
        },
    };
    // Every step names a window defined before it, so the walk ends; the bound guards a WINDOW
    // clause the server would refuse.
    for _ in 0..=named.len() {
        if let Some(frame) = &spec.window_frame {
            return holds_current_row(frame);
        }
        match &spec.window_name {
            None => return true,
            Some(name) => match lookup(name, named) {
                Some(base) => spec = base,
                None => return false,
            },
        }
    }
    false
}

/// The definition of the window of this name in a WINDOW clause, following a definition that is
/// another window's name alone (`w2 AS w1`).
fn lookup<'a>(name: &ast::Ident, named: &'a [ast::NamedWindowDefinition]) -> Option<&'a ast::WindowSpec> {
    let mut name = name;
    for _ in 0..=named.len() {
        let ast::NamedWindowDefinition(_, expr) = named.iter().find(|ast::NamedWindowDefinition(n, _)| n.value.eq_ignore_ascii_case(&name.value))?;
        match expr {
            ast::NamedWindowExpr::WindowSpec(spec) => return Some(spec),
            ast::NamedWindowExpr::NamedWindow(other) => name = other,
        }
    }
    None
}

fn holds_current_row(frame: &ast::WindowFrame) -> bool {
    use ast::{WindowFrameBound as B, WindowFrameUnits as U};
    let offset = |b: &B| matches!(b, B::Preceding(Some(_)) | B::Following(Some(_)));
    let start = matches!(frame.start_bound, B::Preceding(_) | B::CurrentRow);
    let end = matches!(frame.end_bound, None | Some(B::CurrentRow | B::Following(_)));
    let ranged = match frame.units {
        U::Rows => true,
        U::Range => !offset(&frame.start_bound) && !frame.end_bound.as_ref().is_some_and(offset),
        U::Groups => false,
    };
    start && end && ranged
}

/// The statement with each select-list item that is a window call [`windowed`] finds following
/// its argument written as that argument, and the positions of those items and of the ones never
/// NULL: prepared, the copy's flag for such an item is the argument's on the current row, under
/// the same FROM, WHERE and GROUP BY. `None` for anything but a plain SELECT with such an item,
/// where a wildcard makes the positions unreadable, and under a ROLLUP, whose summary rows the
/// window reads too: the server's flags there are left as they are.
pub(crate) fn window_probe(q: &Query) -> Option<(Query, Vec<usize>, Vec<usize>)> {
    let SetExpr::Select(select) = q.body.as_ref() else { return None };
    if select.projection.iter().any(|i| matches!(i, SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(..))) || rolls_up(&select.group_by) {
        return None;
    }
    let (mut follows, mut always) = (Vec::new(), Vec::new());
    let mut projection = select.projection.clone();
    for (i, item) in select.projection.iter().enumerate() {
        let (SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. }) = item else { continue };
        match windowed(e, &select.named_window) {
            Some(Windowed::Follows(arg)) => {
                follows.push(i);
                match &mut projection[i] {
                    SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. } => *e = arg.clone(),
                    _ => {}
                }
            }
            Some(Windowed::Always) => always.push(i),
            None => {}
        }
    }
    if follows.is_empty() && always.is_empty() {
        return None;
    }
    let mut probe = q.clone();
    if let SetExpr::Select(select) = probe.body.as_mut() {
        select.projection = projection;
    }
    Some((probe, follows, always))
}

/// What a select-list item of a SELECT with a plain GROUP BY says of its column when it is an
/// aggregate of its own, not a window call: every group holds a row, so `sum`, `avg`, `min` and
/// `max` (DISTINCT or not) follow their argument over the group's rows, and `json_arrayagg` and
/// `json_objectagg` are never NULL. `None` for anything else.
pub(crate) fn grouped_aggregate(e: &Expr) -> Option<Windowed<'_>> {
    let Expr::Function(f) = strip(e) else { return None };
    if f.over.is_some() || f.filter.is_some() || !f.within_group.is_empty() || f.null_treatment.is_some() {
        return None;
    }
    let ast::FunctionArguments::List(list) = &f.args else { return None };
    if !list.clauses.is_empty() {
        return None;
    }
    let args: Vec<&Expr> = list
        .args
        .iter()
        .map(|a| match a {
            ast::FunctionArg::Unnamed(ast::FunctionArgExpr::Expr(x)) => Some(x),
            _ => None,
        })
        .collect::<Option<_>>()?;
    let name = crate::mysql::builtin(f);
    match (name.as_str(), args.as_slice()) {
        (n, [arg]) if WINDOW_FOLLOWING.contains(&n) => Some(Windowed::Follows(arg)),
        ("json_arrayagg", [_]) | ("json_objectagg", [_, _]) if list.duplicate_treatment.is_none() => Some(Windowed::Always),
        _ => None,
    }
}

/// For a SELECT with a plain GROUP BY: the statement that selects, from the same FROM and WHERE
/// with no GROUP BY, the argument of each select-list item [`grouped_aggregate`] finds following
/// its argument, one column each, beside the positions of those items and of the ones never NULL.
/// Prepared, the copy's flag for a column is the argument's on every row a group aggregates, read
/// from the rows before they are grouped as the aggregate reads them. `None` for anything but a
/// plain SELECT with a plain GROUP BY and such an item, and where a wildcard makes the positions
/// unreadable. A ROLLUP is left to the server's flags, as Postgres leaves a grouping set, though
/// measured on 8.4 it adds no row over no rows and its summary rows aggregate the rows themselves.
pub(crate) fn grouped_probe(q: &Query) -> Option<(Query, Vec<usize>, Vec<usize>)> {
    let SetExpr::Select(select) = q.body.as_ref() else { return None };
    if !grouped(&select.group_by) || select.projection.iter().any(|i| matches!(i, SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(..))) {
        return None;
    }
    let (mut follows, mut always, mut args) = (Vec::new(), Vec::new(), Vec::new());
    for (i, item) in select.projection.iter().enumerate() {
        let (SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias { expr: e, .. }) = item else { continue };
        match grouped_aggregate(e) {
            Some(Windowed::Follows(arg)) => {
                follows.push(i);
                args.push(SelectItem::UnnamedExpr(arg.clone()));
            }
            Some(Windowed::Always) => always.push(i),
            None => {}
        }
    }
    if follows.is_empty() && always.is_empty() {
        return None;
    }
    let mut probe = q.clone();
    probe.order_by = None;
    probe.limit_clause = None;
    probe.locks.clear();
    if let SetExpr::Select(select) = probe.body.as_mut() {
        select.projection = args;
        select.distinct = None;
        select.group_by = GroupByExpr::Expressions(Vec::new(), Vec::new());
        select.having = None;
        select.named_window.clear();
        select.qualify = None;
    }
    Some((probe, follows, always))
}

/// Whether a SELECT has a plain GROUP BY, where every group has a row.
pub(crate) fn grouped(group_by: &GroupByExpr) -> bool {
    matches!(group_by, GroupByExpr::Expressions(exprs, _) if !exprs.is_empty()) && !rolls_up(group_by)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlparser::ast::Statement;

    fn query(sql: &str) -> Query {
        match sqlparser::parser::Parser::parse_sql(&sqlparser::dialect::MySqlDialect {}, sql).unwrap().pop() {
            Some(Statement::Query(q)) => *q,
            other => panic!("{other:?}"),
        }
    }

    fn reduced(sql: &str) -> Option<String> {
        reduce(&query(sql)).map(|q| q.to_string())
    }

    fn strict(test: &str) -> Vec<String> {
        let q = query(&format!("select 1 from t where {test}"));
        let SetExpr::Select(s) = q.body.as_ref() else { unreachable!() };
        strict_in(s.selection.as_ref().unwrap()).into_iter().map(|c| c.join(".")).collect()
    }

    #[test]
    fn a_test_is_strict_where_null_makes_it_never_true() {
        assert_eq!(strict("p.published_at is not null"), ["p.published_at"]);
        assert_eq!(strict("p.id > 0 and u.id < 10"), ["p.id", "u.id"]);
        assert_eq!(strict("p.id = 1 or p.id in (2, 3)"), ["p.id"]);
        assert_eq!(strict("lower(p.title) like 'h%'"), ["p.title"]);
        assert_eq!(strict("p.n + 1 between 1 and 3"), ["p.n"]);
        assert_eq!(strict("not (p.n > 0)"), ["p.n"]);
        assert_eq!(strict("not (p.n is null)"), ["p.n"]);
        assert_eq!(strict("p.flag"), ["p.flag"]);
        assert_eq!(strict("p.flag is true"), ["p.flag"]);
        assert_eq!(strict("p.id in (select id from c)"), ["p.id"]);
        // Through the functions and operators measured strict, at the arguments they are strict in.
        assert_eq!(strict("json_unquote(json_extract(p.doc, '$.t')) = 'x'"), ["p.doc"]);
        assert_eq!(strict("p.doc->>'$.t' = 'x'"), ["p.doc"]);
        assert_eq!(strict("substring(p.title, 1, p.n) = 'x'"), ["p.title", "p.n"]);
        assert_eq!(strict("trim(leading p.pad from p.title) = 'x'"), ["p.title", "p.pad"]);
        assert_eq!(strict("timestampdiff(day, p.at, now()) > 1"), ["p.at"]);
        assert_eq!(strict("(p.n div 2 = 1) xor (p.m & 1)"), ["p.n", "p.m"]);
        assert_eq!(strict("p.n member of (p.doc)"), ["p.n", "p.doc"]);
        assert_eq!(strict("date_add(p.at, interval p.n day) < now()"), ["p.at", "p.n"]);
        assert_eq!(strict("p.id in (1, p.n)"), ["p.id"]);
        assert_eq!(strict("p.n between p.lo and p.hi"), ["p.n"]);
        assert_eq!(strict("concat_ws(p.sep, p.title) = 'x'"), ["p.sep"]);
        // What NULL can pass: an OR whose arms test different columns, `IS NULL`, `<=>`,
        // `coalesce`, `IS NOT TRUE`, NOT over a test that is true for NULL, and a `NOT IN` over a
        // subquery, which is true for NULL where the subquery is empty.
        for test in [
            "p.published_at is not null or u.email is not null",
            "p.id is null",
            "p.title <=> 'x'",
            "not (p.title <=> 'x')",
            "coalesce(p.published_at, '2000-01-01') > '1999-01-01'",
            "ifnull(p.n, 0) = 0",
            "p.flag is not true",
            "not (p.n is not null)",
            "not (p.n > 0 and p.m > 0)",
            "p.id not in (select id from c)",
            "concat_ws(',', p.title) = 'x'",
            "nullif('x', p.title) = 'x'",
            "json_set('{}', '$.a', p.n) is not null",
            "week('2026-01-01', p.mode) = 1",
            "if(p.n > 0, 1, 0) = 0",
            "char(p.n) = ''",
            "1 between p.lo and 2",
            "`lower`(p.title) = 'x'",
        ] {
            assert_eq!(strict(test), Vec::<String>::new(), "{test}");
        }
    }

    #[test]
    fn the_strict_functions_are_named_once_in_lower_case() {
        let mut names: Vec<&str> = STRICT_FUNCTIONS.iter().map(|(n, _)| *n).collect();
        assert!(names.iter().all(|n| *n == n.to_ascii_lowercase()));
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), STRICT_FUNCTIONS.len());
    }

    #[test]
    fn an_outer_join_whose_null_extension_a_strict_test_rejects_is_inner() {
        assert_eq!(
            reduced("select u.name, p.title from users u left join posts p on p.user_id = u.id where p.published_at is not null").as_deref(),
            Some("SELECT u.name, p.title FROM users u INNER JOIN posts p ON p.user_id = u.id WHERE p.published_at IS NOT NULL")
        );
        // The left side of a RIGHT JOIN, inside a nested join, and an inner join's ON clause,
        // which rejects the null extension of the joins before it.
        assert!(reduced("select 1 from posts p right join users u on p.user_id = u.id where p.title like 'h%'").is_some_and(|s| !s.contains("RIGHT")));
        assert!(reduced("select 1 from users u left join (posts p left join comments c on c.post_id = p.id) on p.user_id = u.id where c.id > 0").is_some_and(|s| !s.contains("LEFT")));
        assert!(reduced("select 1 from comments c left join posts p on p.id = c.post_id join users u on u.id = p.user_id").is_some_and(|s| !s.contains("LEFT")));
        // A test of the other side, a test that is not strict, an unqualified column (the
        // engine does not know whose it is), a LEFT JOIN's own ON, and no WHERE prove nothing.
        for sql in [
            "select 1 from users u left join posts p on p.user_id = u.id where u.email is not null",
            "select 1 from users u left join posts p on p.user_id = u.id where p.id is null",
            "select 1 from users u left join posts p on p.user_id = u.id where p.id > 0 or u.id > 0",
            "select 1 from users u left join posts p on p.user_id = u.id where coalesce(p.id, 0) = 0",
            "select 1 from users u left join posts p on p.user_id = u.id where published_at is not null",
            "select 1 from comments c left join posts p on p.id = c.post_id left join users u on u.id = p.user_id",
            "select 1 from users u left join posts p on p.user_id = u.id",
            "select 1 from users u left join posts p on p.user_id = u.id union select 2",
        ] {
            assert_eq!(reduced(sql), None, "{sql}");
        }
        // A later LEFT JOIN made inner by the WHERE lends its ON clause to the one before it.
        assert!(reduced("select 1 from comments c left join posts p on p.id = c.post_id left join users u on u.id = p.user_id where u.name > ''").is_some_and(|s| !s.contains("LEFT")));
    }

    #[test]
    fn an_aggregate_under_a_plain_group_by_follows_its_argument() {
        let probe = |sql: &str| grouped_probe(&query(sql)).map(|(q, follows, always)| (q.to_string(), follows, always));
        // The arguments, read from the rows before they are grouped, and the JSON aggregates.
        assert_eq!(
            probe("select distinct user_id, sum(a) as s, max(distinct b), count(*), json_arrayagg(b) as j, json_objectagg(a, b) from t where a > 0 group by user_id having count(*) > 1 order by s limit 2"),
            Some(("SELECT a, b FROM t WHERE a > 0".into(), vec![1, 2], vec![4, 5]))
        );
        assert_eq!(probe("with w as (select a from t) select a, min(a) from w group by a"), Some(("WITH w AS (SELECT a FROM t) SELECT a FROM w".into(), vec![1], vec![])));
        // No GROUP BY, a ROLLUP, a window call, a wildcard, an expression over an aggregate and
        // the aggregates that are NULL otherwise prove nothing.
        for sql in [
            "select sum(a) from t",
            "select user_id, sum(a) from t group by user_id with rollup",
            "select user_id, sum(a) over () from t group by user_id, a",
            "select *, sum(a) from t group by user_id",
            "select user_id, sum(a) + 1 from t group by user_id",
            "select user_id, var_samp(a), stddev_samp(a), group_concat(a), `sum`(a), app.sum(a) from t group by user_id",
            "select user_id, a from t union all select user_id, max(a) from t group by user_id",
        ] {
            assert_eq!(probe(sql), None, "{sql}");
        }
    }
}
