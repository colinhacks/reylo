//! The C functions the grammar's actions call: `gram.y`'s own, `makefuncs.c`'s and the list
//! functions, with the semantics they have there, a list growing in place included.

use super::generated_names::{c, f, t};
use super::nodes::{R, V};
use std::cell::RefCell;

thread_local! {
    static EXTRA: RefCell<V> = const { RefCell::new(V::Null) };
}

/// The parser's extra state, whose `parsetree` the start rule sets.
pub fn pg_yyget_extra(_: &V) -> R {
    EXTRA.with(|e| {
        let mut e = e.borrow_mut();
        if !e.truthy() {
            *e = V::node(t::T_RawStmt);
        }
        Ok(e.clone())
    })
}

/// Clears that state, before a parse.
pub fn reset_extra() {
    EXTRA.with(|e| *e.borrow_mut() = V::Null);
}

/// What the start rule left there.
pub fn parsetree() -> V {
    EXTRA.with(|e| e.borrow().get(f::F_parsetree))
}

fn text(v: &V) -> String {
    match v {
        V::Str(s) => String::from_utf8_lossy(s).into_owned(),
        _ => String::new(),
    }
}

/// A C format with `%s`, `%d`, `%u`, `%c` and `%%`.
pub fn message(format: &str, args: &[V]) -> String {
    let (mut out, mut rest, mut arg) = (String::new(), format.chars().peekable(), args.iter());
    while let Some(c) = rest.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match rest.next() {
            Some('%') => out.push('%'),
            Some('s') => out.push_str(&arg.next().map(text).unwrap_or_default()),
            Some('d' | 'u') => out.push_str(&arg.next().map_or(0, V::int).to_string()),
            Some('c') => out.push(arg.next().map_or(0, V::int) as u8 as char),
            other => {
                out.push('%');
                out.extend(other);
            }
        }
    }
    out
}

pub fn psprintf(args: &[V]) -> R {
    let format = args.first().map(text).unwrap_or_default();
    Ok(V::s(&message(&format, &args[1..])))
}

/// Marks a refusal in `scanner_yyerror`'s words, which go on to name the last token read.
pub const AT_TOKEN: &str = "\u{1}";

fn at_token(words: &str) -> String {
    format!("{AT_TOKEN}{words}")
}

/// `parser_yyerror`.
pub fn syntax(what: V, _location: i32) -> String {
    at_token(&text(&what))
}

pub fn strcmp(a: &V, b: &V) -> R {
    let bytes = |v: &V| match v {
        V::Str(s) => s.to_vec(),
        _ => Vec::new(),
    };
    Ok(V::Int(match bytes(a).cmp(&bytes(b)) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }))
}

pub fn pg_strcasecmp(a: &V, b: &V) -> R {
    Ok(V::b(!text(a).eq_ignore_ascii_case(&text(b))))
}

#[allow(non_snake_case)]
pub fn INTERVAL_MASK(b: &V) -> R {
    Ok(V::Int(1 << b.int()))
}

// Lists.

pub fn list_make1(a: &V) -> R {
    Ok(V::list(vec![a.clone()]))
}
pub fn list_make2(a: &V, b: &V) -> R {
    Ok(V::list(vec![a.clone(), b.clone()]))
}
pub fn list_make3(a: &V, b: &V, d: &V) -> R {
    Ok(V::list(vec![a.clone(), b.clone(), d.clone()]))
}
pub fn list_make4(a: &V, b: &V, d: &V, e: &V) -> R {
    Ok(V::list(vec![a.clone(), b.clone(), d.clone(), e.clone()]))
}

pub fn lappend(list: &V, item: &V) -> R {
    match list {
        V::List(l) => {
            l.borrow_mut().push(item.clone());
            Ok(list.clone())
        }
        _ => list_make1(item),
    }
}

pub fn lcons(item: &V, list: &V) -> R {
    match list {
        V::List(l) => {
            l.borrow_mut().insert(0, item.clone());
            Ok(list.clone())
        }
        _ => list_make1(item),
    }
}

/// Appends `b`'s cells to `a`, in place; with `a` empty, a copy of `b`.
pub fn list_concat(a: &V, b: &V) -> R {
    if b.is_empty() {
        return Ok(a.clone());
    }
    match a {
        V::List(l) if !l.borrow().is_empty() => {
            let items = b.items();
            l.borrow_mut().extend(items);
            Ok(a.clone())
        }
        _ => Ok(V::list(b.items())),
    }
}

pub fn list_length(list: &V) -> R {
    Ok(V::Int(list.len() as i64))
}

fn nth(list: &V, n: usize) -> V {
    match list {
        V::List(l) => l.borrow().get(n).cloned().unwrap_or(V::Null),
        _ => V::Null,
    }
}

pub fn linitial(list: &V) -> V {
    nth(list, 0)
}
pub fn lsecond(list: &V) -> V {
    nth(list, 1)
}
pub fn lfirst(cell: &V) -> V {
    cell.deref()
}
pub fn llast(list: &V) -> V {
    nth(list, list.len().wrapping_sub(1))
}

fn set_nth(list: &V, n: usize, value: V) {
    if let V::List(l) = list {
        if let Some(slot) = l.borrow_mut().get_mut(n) {
            *slot = value;
        }
    }
}

pub fn set_linitial(list: &V, value: V) {
    set_nth(list, 0, value);
}

// Values.

fn value_node(tag: u16, field: u16, value: &V) -> R {
    let n = V::node(tag);
    n.set(field, value.clone());
    Ok(n)
}

#[allow(non_snake_case)]
pub fn makeString(s: &V) -> R {
    value_node(t::T_String, f::F_sval, s)
}
#[allow(non_snake_case)]
pub fn makeInteger(i: &V) -> R {
    value_node(t::T_Integer, f::F_ival, i)
}
#[allow(non_snake_case)]
pub fn makeFloat(s: &V) -> R {
    value_node(t::T_Float, f::F_fval, s)
}
#[allow(non_snake_case)]
pub fn makeBoolean(b: &V) -> R {
    value_node(t::T_Boolean, f::F_boolval, b)
}

#[allow(non_snake_case)]
pub fn strVal(v: &V) -> R {
    Ok(v.get(f::F_sval))
}
#[allow(non_snake_case)]
pub fn intVal(v: &V) -> R {
    Ok(v.get(f::F_ival))
}
#[allow(non_snake_case)]
pub fn boolVal(v: &V) -> R {
    Ok(v.get(f::F_boolval))
}

#[allow(non_snake_case)]
pub fn defGetInt32(def: &V) -> R {
    let arg = def.get(f::F_arg);
    if !arg.truthy() {
        return Err(format!("{} requires an integer value", text(&def.get(f::F_defname))));
    }
    if arg.is_a(t::T_Integer) {
        return Ok(arg.get(f::F_ival));
    }
    Err(format!("{} requires an integer value", text(&def.get(f::F_defname))))
}

#[allow(non_snake_case)]
pub fn NameListToString(names: &V) -> R {
    let parts: Vec<String> = names
        .items()
        .iter()
        .map(|n| if n.is_a(t::T_A_Star) { "*".to_string() } else { text(&n.get(f::F_sval)) })
        .collect();
    Ok(V::s(&parts.join(".")))
}

// Constants.

fn a_const(value: V, location: &V) -> R {
    let n = V::node(t::T_A_Const);
    n.set(f::F_val, value);
    n.set(f::F_location, location.clone());
    Ok(n)
}

#[allow(non_snake_case)]
pub fn makeStringConst(s: &V, location: &V) -> R {
    a_const(makeString(s)?, location)
}
#[allow(non_snake_case)]
pub fn makeIntConst(i: &V, location: &V) -> R {
    a_const(makeInteger(i)?, location)
}
#[allow(non_snake_case)]
pub fn makeFloatConst(s: &V, location: &V) -> R {
    a_const(makeFloat(s)?, location)
}
#[allow(non_snake_case)]
pub fn makeBoolAConst(b: &V, location: &V) -> R {
    a_const(makeBoolean(b)?, location)
}
#[allow(non_snake_case)]
pub fn makeBitStringConst(s: &V, location: &V) -> R {
    a_const(value_node(t::T_BitString, f::F_bsval, s)?, location)
}
#[allow(non_snake_case)]
pub fn makeNullAConst(location: &V) -> R {
    let n = V::node(t::T_A_Const);
    n.set(f::F_isnull, V::Int(1));
    n.set(f::F_location, location.clone());
    Ok(n)
}
#[allow(non_snake_case)]
pub fn makeAConst(v: &V, location: &V) -> R {
    if v.is_a(t::T_Float) {
        makeFloatConst(&v.get(f::F_fval), location)
    } else if v.is_a(t::T_Integer) {
        makeIntConst(&v.get(f::F_ival), location)
    } else {
        Ok(V::Null)
    }
}
#[allow(non_snake_case)]
pub fn makeParamRef(number: &V, location: &V) -> R {
    let p = V::node(t::T_ParamRef);
    p.set(f::F_number, number.clone());
    p.set(f::F_location, location.clone());
    Ok(p)
}
#[allow(non_snake_case)]
pub fn makeParamRefCast(number: &V, location: &V, type_name: &V) -> R {
    makeTypeCast(&makeParamRef(number, location)?, type_name, &V::Int(-1))
}
#[allow(non_snake_case)]
pub fn makeTypeCast(arg: &V, type_name: &V, location: &V) -> R {
    let n = V::node(t::T_TypeCast);
    n.set(f::F_arg, arg.clone());
    n.set(f::F_typeName, type_name.clone());
    n.set(f::F_location, location.clone());
    Ok(n)
}
#[allow(non_snake_case)]
pub fn makeStringConstCast(s: &V, location: &V, type_name: &V) -> R {
    makeTypeCast(&makeStringConst(s, location)?, type_name, &V::Int(-1))
}
#[allow(non_snake_case)]
pub fn makeRoleSpec(kind: &V, location: &V) -> R {
    let n = V::node(t::T_RoleSpec);
    n.set(f::F_roletype, kind.clone());
    n.set(f::F_location, location.clone());
    Ok(n)
}

// makefuncs.c.

#[allow(non_snake_case)]
pub fn makeA_Expr(kind: &V, name: &V, lexpr: &V, rexpr: &V, location: &V) -> R {
    let a = V::node(t::T_A_Expr);
    a.set(f::F_kind, kind.clone());
    a.set(f::F_name, name.clone());
    a.set(f::F_lexpr, lexpr.clone());
    a.set(f::F_rexpr, rexpr.clone());
    a.set(f::F_location, location.clone());
    Ok(a)
}
#[allow(non_snake_case)]
pub fn makeSimpleA_Expr(kind: &V, name: &V, lexpr: &V, rexpr: &V, location: &V) -> R {
    makeA_Expr(kind, &list_make1(&makeString(name)?)?, lexpr, rexpr, location)
}
#[allow(non_snake_case)]
pub fn makeBoolExpr(op: &V, args: &V, location: &V) -> R {
    let b = V::node(t::T_BoolExpr);
    b.set(f::F_boolop, op.clone());
    b.set(f::F_args, args.clone());
    b.set(f::F_location, location.clone());
    Ok(b)
}
#[allow(non_snake_case)]
pub fn makeAlias(name: &V, colnames: &V) -> R {
    let a = V::node(t::T_Alias);
    a.set(f::F_aliasname, name.clone());
    a.set(f::F_colnames, colnames.clone());
    Ok(a)
}
#[allow(non_snake_case)]
pub fn makeRangeVar(schema: &V, rel: &V, location: &V) -> R {
    let r = V::node(t::T_RangeVar);
    r.set(f::F_schemaname, schema.clone());
    r.set(f::F_relname, rel.clone());
    r.set(f::F_inh, V::Int(1));
    r.set(f::F_relpersistence, V::Int(c::RELPERSISTENCE_PERMANENT));
    r.set(f::F_location, location.clone());
    Ok(r)
}
#[allow(non_snake_case)]
pub fn makeTypeNameFromNameList(names: &V) -> R {
    let n = V::node(t::T_TypeName);
    n.set(f::F_names, names.clone());
    n.set(f::F_typemod, V::Int(-1));
    n.set(f::F_location, V::Int(-1));
    Ok(n)
}
#[allow(non_snake_case)]
pub fn makeTypeName(name: &V) -> R {
    makeTypeNameFromNameList(&list_make1(&makeString(name)?)?)
}
#[allow(non_snake_case)]
pub fn makeDefElem(name: &V, arg: &V, location: &V) -> R {
    let d = V::node(t::T_DefElem);
    d.set(f::F_defname, name.clone());
    d.set(f::F_arg, arg.clone());
    d.set(f::F_defaction, V::Int(c::DEFELEM_UNSPEC));
    d.set(f::F_location, location.clone());
    d.set(f::F_arg_location, V::Int(-1));
    Ok(d)
}
#[allow(non_snake_case)]
pub fn makeDefElemExtended(namespace: &V, name: &V, arg: &V, action: &V, location: &V) -> R {
    let d = makeDefElem(name, arg, location)?;
    d.set(f::F_defnamespace, namespace.clone());
    d.set(f::F_defaction, action.clone());
    Ok(d)
}
#[allow(non_snake_case)]
pub fn makeFuncCall(name: &V, args: &V, format: &V, location: &V) -> R {
    let n = V::node(t::T_FuncCall);
    n.set(f::F_funcname, name.clone());
    n.set(f::F_args, args.clone());
    n.set(f::F_funcformat, format.clone());
    n.set(f::F_location, location.clone());
    Ok(n)
}
#[allow(non_snake_case)]
pub fn makeGroupingSet(kind: &V, content: &V, location: &V) -> R {
    let n = V::node(t::T_GroupingSet);
    n.set(f::F_kind, kind.clone());
    n.set(f::F_content, content.clone());
    n.set(f::F_location, location.clone());
    Ok(n)
}
#[allow(non_snake_case)]
pub fn makeVacuumRelation(relation: &V, oid: &V, cols: &V) -> R {
    let v = V::node(t::T_VacuumRelation);
    v.set(f::F_relation, relation.clone());
    v.set(f::F_oid, oid.clone());
    v.set(f::F_va_cols, cols.clone());
    Ok(v)
}
#[allow(non_snake_case)]
pub fn makeJsonFormat(kind: &V, encoding: &V, location: &V) -> R {
    let j = V::node(t::T_JsonFormat);
    j.set(f::F_format_type, kind.clone());
    j.set(f::F_encoding, encoding.clone());
    j.set(f::F_location, location.clone());
    Ok(j)
}
#[allow(non_snake_case)]
pub fn makeJsonValueExpr(raw: &V, formatted: &V, format: &V) -> R {
    let j = V::node(t::T_JsonValueExpr);
    j.set(f::F_raw_expr, raw.clone());
    j.set(f::F_formatted_expr, formatted.clone());
    j.set(f::F_format, format.clone());
    Ok(j)
}
#[allow(non_snake_case)]
pub fn makeJsonBehavior(kind: &V, expr: &V, location: &V) -> R {
    let j = V::node(t::T_JsonBehavior);
    j.set(f::F_btype, kind.clone());
    j.set(f::F_expr, expr.clone());
    j.set(f::F_location, location.clone());
    Ok(j)
}
#[allow(non_snake_case)]
pub fn makeJsonKeyValue(key: &V, value: &V) -> R {
    let j = V::node(t::T_JsonKeyValue);
    j.set(f::F_key, key.clone());
    j.set(f::F_value, value.clone());
    Ok(j)
}
#[allow(non_snake_case)]
pub fn makeJsonIsPredicate(expr: &V, format: &V, item_type: &V, unique: &V, location: &V) -> R {
    let j = V::node(t::T_JsonIsPredicate);
    j.set(f::F_expr, expr.clone());
    j.set(f::F_format, format.clone());
    j.set(f::F_item_type, item_type.clone());
    j.set(f::F_unique_keys, unique.clone());
    j.set(f::F_location, location.clone());
    Ok(j)
}
#[allow(non_snake_case)]
pub fn makeJsonTablePathSpec(string: &V, name: &V, string_location: &V, name_location: &V) -> R {
    let j = V::node(t::T_JsonTablePathSpec);
    j.set(f::F_string, makeStringConst(string, string_location)?);
    if name.truthy() {
        j.set(f::F_name, name.clone());
    }
    j.set(f::F_name_location, name_location.clone());
    j.set(f::F_location, string_location.clone());
    Ok(j)
}

// gram.y's own.

#[allow(non_snake_case)]
pub fn makeRawStmt(stmt: &V, location: &V) -> R {
    let r = V::node(t::T_RawStmt);
    r.set(f::F_stmt, stmt.clone());
    r.set(f::F_stmt_location, location.clone());
    r.set(f::F_stmt_len, V::Int(0));
    Ok(r)
}

#[allow(non_snake_case)]
pub fn updateRawStmtEnd(raw: &V, end: &V) -> R {
    if raw.get(f::F_stmt_len).int() <= 0 {
        raw.set(f::F_stmt_len, V::Int(end.int() - raw.get(f::F_stmt_location).int()));
    }
    Ok(V::Null)
}

/// `check_indirection`'s rule: `*` only last.
fn star_only_last(list: &V) -> Result<(), String> {
    let items = list.items();
    for (i, item) in items.iter().enumerate() {
        if item.is_a(t::T_A_Star) && i + 1 != items.len() {
            return Err(at_token("improper use of \"*\""));
        }
    }
    Ok(())
}

#[allow(non_snake_case)]
pub fn makeColumnRef(colname: &V, indirection: &V, location: &V, _: &V) -> R {
    let col = V::node(t::T_ColumnRef);
    col.set(f::F_location, location.clone());
    let items = indirection.items();
    let mut nfields = 0usize;
    for (i, item) in items.iter().enumerate() {
        if item.is_a(t::T_A_Indices) {
            let ind = V::node(t::T_A_Indirection);
            if nfields == 0 {
                col.set(f::F_fields, list_make1(&makeString(colname)?)?);
                star_only_last(indirection)?;
                ind.set(f::F_indirection, indirection.clone());
            } else {
                let tail = V::list(items[nfields..].to_vec());
                star_only_last(&tail)?;
                ind.set(f::F_indirection, tail);
                let head = V::list(items[..nfields].to_vec());
                col.set(f::F_fields, lcons(&makeString(colname)?, &head)?);
            }
            ind.set(f::F_arg, col);
            return Ok(ind);
        } else if item.is_a(t::T_A_Star) && i + 1 != items.len() {
            return Err(at_token("improper use of \"*\""));
        }
        nfields += 1;
    }
    col.set(f::F_fields, lcons(&makeString(colname)?, indirection)?);
    Ok(col)
}

#[allow(non_snake_case)]
pub fn check_qualified_name(names: &V, _: &V) -> R {
    if names.items().iter().any(|n| !n.is_a(t::T_String)) {
        return Err(at_token("syntax error"));
    }
    Ok(V::Null)
}

pub fn check_func_name(names: &V, _: &V) -> R {
    if names.items().iter().any(|n| !n.is_a(t::T_String)) {
        return Err(at_token("syntax error"));
    }
    Ok(names.clone())
}

pub fn check_indirection(list: &V, _: &V) -> R {
    star_only_last(list)?;
    Ok(list.clone())
}

#[allow(non_snake_case)]
pub fn extractArgTypes(parameters: &V) -> R {
    let mut result = V::Null;
    for p in parameters.items() {
        let mode = p.get(f::F_mode).int();
        if mode != c::FUNC_PARAM_OUT && mode != c::FUNC_PARAM_TABLE {
            result = lappend(&result, &p.get(f::F_argType))?;
        }
    }
    Ok(result)
}

#[allow(non_snake_case)]
pub fn extractAggrArgTypes(args: &V) -> R {
    extractArgTypes(&linitial(args))
}

/// `equal()` over the parts of a `TypeName` an ordered-set aggregate compares.
fn equal(a: &V, b: &V) -> bool {
    match (a, b) {
        (V::Null, V::Null) => true,
        (V::Int(x), V::Int(y)) => x == y,
        (V::Str(x), V::Str(y)) => x == y,
        (V::List(x), V::List(y)) => {
            let (x, y) = (x.borrow(), y.borrow());
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(p, q)| equal(p, q))
        }
        (V::Node(_), V::Node(_)) => {
            if a.tag() != b.tag() {
                return false;
            }
            [f::F_names, f::F_typeOid, f::F_setof, f::F_pct_type, f::F_typmods, f::F_typemod, f::F_arrayBounds, f::F_sval, f::F_ival, f::F_val]
                .iter()
                .all(|field| equal(&a.get(*field), &b.get(*field)))
        }
        (V::Null, V::Int(0)) | (V::Int(0), V::Null) => true,
        _ => false,
    }
}

#[allow(non_snake_case)]
pub fn makeOrderedSetArgs(direct: &V, ordered: &V, _: &V) -> R {
    let last = llast(direct);
    let mut ordered = ordered.clone();
    if last.get(f::F_mode).int() == c::FUNC_PARAM_VARIADIC {
        let first = linitial(&ordered);
        if ordered.len() != 1 || first.get(f::F_mode).int() != c::FUNC_PARAM_VARIADIC || !equal(&last.get(f::F_argType), &first.get(f::F_argType)) {
            return Err("an ordered-set aggregate with a VARIADIC direct argument must have one VARIADIC aggregated argument of the same data type".to_string());
        }
        ordered = V::Null;
    }
    let count = makeInteger(&V::Int(direct.len() as i64))?;
    list_make2(&list_concat(direct, &ordered)?, &count)
}

#[allow(non_snake_case)]
pub fn insertSelectOptions(stmt: &V, sort: &V, locking: &V, limit: &V, with: &V, _: &V) -> R {
    if sort.truthy() {
        if stmt.get(f::F_sortClause).truthy() {
            return Err("multiple ORDER BY clauses not allowed".to_string());
        }
        stmt.set(f::F_sortClause, sort.clone());
    }
    stmt.set(f::F_lockingClause, list_concat(&stmt.get(f::F_lockingClause), locking)?);
    if limit.truthy() && limit.get(f::F_limitOffset).truthy() {
        if stmt.get(f::F_limitOffset).truthy() {
            return Err("multiple OFFSET clauses not allowed".to_string());
        }
        stmt.set(f::F_limitOffset, limit.get(f::F_limitOffset));
    }
    if limit.truthy() && limit.get(f::F_limitCount).truthy() {
        if stmt.get(f::F_limitCount).truthy() {
            return Err("multiple LIMIT clauses not allowed".to_string());
        }
        stmt.set(f::F_limitCount, limit.get(f::F_limitCount));
    }
    if limit.truthy() {
        let option = limit.get(f::F_limitOption).int();
        if !stmt.get(f::F_sortClause).truthy() && option == c::LIMIT_OPTION_WITH_TIES {
            return Err("WITH TIES cannot be specified without ORDER BY clause".to_string());
        }
        if option == c::LIMIT_OPTION_WITH_TIES {
            for lock in stmt.get(f::F_lockingClause).items() {
                if lock.get(f::F_waitPolicy).int() == c::LockWaitSkip {
                    return Err("SKIP LOCKED and WITH TIES options cannot be used together".to_string());
                }
            }
        }
        stmt.set(f::F_limitOption, V::Int(option));
    }
    if with.truthy() {
        if stmt.get(f::F_withClause).truthy() {
            return Err("multiple WITH clauses not allowed".to_string());
        }
        stmt.set(f::F_withClause, with.clone());
    }
    Ok(V::Null)
}

#[allow(non_snake_case)]
pub fn makeSetOp(op: &V, all: &V, larg: &V, rarg: &V) -> R {
    let n = V::node(t::T_SelectStmt);
    n.set(f::F_op, op.clone());
    n.set(f::F_all, all.clone());
    n.set(f::F_larg, larg.clone());
    n.set(f::F_rarg, rarg.clone());
    Ok(n)
}

#[allow(non_snake_case)]
pub fn SystemFuncName(name: &V) -> R {
    list_make2(&makeString(&V::s("pg_catalog"))?, &makeString(name)?)
}

#[allow(non_snake_case)]
pub fn SystemTypeName(name: &V) -> R {
    makeTypeNameFromNameList(&list_make2(&makeString(&V::s("pg_catalog"))?, &makeString(name)?)?)
}

#[allow(non_snake_case)]
pub fn doNegateFloat(v: &V) -> R {
    let old = text(&v.get(f::F_fval));
    let old = old.strip_prefix('+').unwrap_or(&old);
    let new = match old.strip_prefix('-') {
        Some(rest) => rest.to_string(),
        None => format!("-{old}"),
    };
    v.set(f::F_fval, V::s(&new));
    Ok(V::Null)
}

#[allow(non_snake_case)]
pub fn doNegate(n: &V, location: &V) -> R {
    if n.is_a(t::T_A_Const) {
        n.set(f::F_location, location.clone());
        let value = n.get(f::F_val);
        if value.is_a(t::T_Integer) {
            value.set(f::F_ival, V::Int(-value.get(f::F_ival).int()));
            return Ok(n.clone());
        }
        if value.is_a(t::T_Float) {
            doNegateFloat(&value)?;
            return Ok(n.clone());
        }
    }
    makeSimpleA_Expr(&V::Int(c::AEXPR_OP), &V::s("-"), &V::Null, n, location)
}

fn flattened(kind: i64, l: &V, r: &V, location: &V) -> R {
    if l.is_a(t::T_BoolExpr) && l.get(f::F_boolop).int() == kind {
        l.set(f::F_args, lappend(&l.get(f::F_args), r)?);
        return Ok(l.clone());
    }
    makeBoolExpr(&V::Int(kind), &list_make2(l, r)?, location)
}

#[allow(non_snake_case)]
pub fn makeAndExpr(l: &V, r: &V, location: &V) -> R {
    flattened(c::AND_EXPR, l, r, location)
}
#[allow(non_snake_case)]
pub fn makeOrExpr(l: &V, r: &V, location: &V) -> R {
    flattened(c::OR_EXPR, l, r, location)
}
#[allow(non_snake_case)]
pub fn makeNotExpr(e: &V, location: &V) -> R {
    makeBoolExpr(&V::Int(c::NOT_EXPR), &list_make1(e)?, location)
}

#[allow(non_snake_case)]
pub fn makeAArrayExpr(elements: &V, location: &V, end: &V) -> R {
    let n = V::node(t::T_A_ArrayExpr);
    n.set(f::F_elements, elements.clone());
    n.set(f::F_location, location.clone());
    n.set(f::F_list_start, location.clone());
    n.set(f::F_list_end, end.clone());
    Ok(n)
}

#[allow(non_snake_case)]
pub fn makeSQLValueFunction(op: &V, typmod: &V, location: &V) -> R {
    let n = V::node(t::T_SQLValueFunction);
    n.set(f::F_op, op.clone());
    n.set(f::F_typmod, typmod.clone());
    n.set(f::F_location, location.clone());
    Ok(n)
}

#[allow(non_snake_case)]
pub fn makeXmlExpr(op: &V, name: &V, named_args: &V, args: &V, location: &V) -> R {
    let n = V::node(t::T_XmlExpr);
    n.set(f::F_op, op.clone());
    n.set(f::F_name, name.clone());
    n.set(f::F_named_args, named_args.clone());
    n.set(f::F_args, args.clone());
    n.set(f::F_location, location.clone());
    Ok(n)
}

#[allow(non_snake_case)]
pub fn mergeTableFuncParameters(args: &V, columns: &V, _: &V) -> R {
    for p in args.items() {
        let mode = p.get(f::F_mode).int();
        if mode != c::FUNC_PARAM_DEFAULT && mode != c::FUNC_PARAM_IN && mode != c::FUNC_PARAM_VARIADIC {
            return Err("OUT and INOUT arguments aren't allowed in TABLE functions".to_string());
        }
    }
    list_concat(args, columns)
}

#[allow(non_snake_case)]
pub fn TableFuncTypeName(columns: &V) -> R {
    let result = if columns.len() == 1 { linitial(columns).get(f::F_argType).copied() } else { SystemTypeName(&V::s("record"))? };
    result.set(f::F_setof, V::Int(1));
    Ok(result)
}

#[allow(non_snake_case)]
pub fn makeRangeVarFromAnyName(names: &V, position: &V, _: &V) -> R {
    let r = V::node(t::T_RangeVar);
    let items = names.items();
    let val = |i: usize| items[i].get(f::F_sval);
    match items.len() {
        1 => r.set(f::F_relname, val(0)),
        2 => {
            r.set(f::F_schemaname, val(0));
            r.set(f::F_relname, val(1));
        }
        3 => {
            r.set(f::F_catalogname, val(0));
            r.set(f::F_schemaname, val(1));
            r.set(f::F_relname, val(2));
        }
        _ => return Err(format!("improper qualified name (too many dotted names): {}", text(&NameListToString(names)?))),
    }
    r.set(f::F_relpersistence, V::Int(c::RELPERSISTENCE_PERMANENT));
    r.set(f::F_location, position.clone());
    Ok(r)
}

#[allow(non_snake_case)]
pub fn makeRangeVarFromQualifiedName(name: &V, names: &V, location: &V, scanner: &V) -> R {
    check_qualified_name(names, scanner)?;
    let r = makeRangeVar(&V::Null, &V::Null, location)?;
    let items = names.items();
    match items.len() {
        1 => {
            r.set(f::F_schemaname, name.clone());
            r.set(f::F_relname, items[0].get(f::F_sval));
        }
        2 => {
            r.set(f::F_catalogname, name.clone());
            r.set(f::F_schemaname, items[0].get(f::F_sval));
            r.set(f::F_relname, items[1].get(f::F_sval));
        }
        _ => {
            let all = V::list(std::iter::once(makeString(name)?).chain(items).collect());
            return Err(format!("improper qualified name (too many dotted names): {}", text(&NameListToString(&all)?)));
        }
    }
    Ok(r)
}

#[allow(non_snake_case)]
pub fn SplitColQualList(quals: &V, constraints: &V, collation: &V, _: &V) -> R {
    collation.store(V::Null);
    let mut kept = Vec::new();
    for n in quals.items() {
        if n.is_a(t::T_Constraint) {
            kept.push(n);
        } else if n.is_a(t::T_CollateClause) {
            if collation.deref().truthy() {
                return Err("multiple COLLATE clauses not allowed".to_string());
            }
            collation.store(n);
        } else {
            return Err("unexpected node type".to_string());
        }
    }
    constraints.store(if kept.is_empty() { V::Null } else { V::list(kept) });
    Ok(V::Null)
}

#[allow(non_snake_case, clippy::too_many_arguments)]
pub fn processCASbits(bits: &V, _location: &V, kind: &V, deferrable: &V, initdeferred: &V, is_enforced: &V, not_valid: &V, no_inherit: &V, _: &V) -> R {
    let bits = bits.int();
    let name = text(kind);
    let given = |p: &V| matches!(p, V::Place(..));
    let set = |p: &V, v: i64| p.store(V::Int(v));
    set(deferrable, 0);
    set(initdeferred, 0);
    set(not_valid, 0);
    set(is_enforced, 1);
    if bits & (c::CAS_DEFERRABLE | c::CAS_INITIALLY_DEFERRED) != 0 {
        if !given(deferrable) {
            return Err(format!("{name} constraints cannot be marked DEFERRABLE"));
        }
        set(deferrable, 1);
    }
    if bits & c::CAS_INITIALLY_DEFERRED != 0 {
        if !given(initdeferred) {
            return Err(format!("{name} constraints cannot be marked DEFERRABLE"));
        }
        set(initdeferred, 1);
    }
    if bits & c::CAS_NOT_VALID != 0 {
        if !given(not_valid) {
            return Err(format!("{name} constraints cannot be marked NOT VALID"));
        }
        set(not_valid, 1);
    }
    if bits & c::CAS_NO_INHERIT != 0 {
        if !given(no_inherit) {
            return Err(format!("{name} constraints cannot be marked NO INHERIT"));
        }
        set(no_inherit, 1);
    }
    if bits & c::CAS_NOT_ENFORCED != 0 {
        if !given(is_enforced) {
            return Err(format!("{name} constraints cannot be marked NOT ENFORCED"));
        }
        set(is_enforced, 0);
        if given(not_valid) {
            set(not_valid, 1);
        }
    }
    if bits & c::CAS_ENFORCED != 0 {
        if !given(is_enforced) {
            return Err(format!("{name} constraints cannot be marked ENFORCED"));
        }
        set(is_enforced, 1);
    }
    Ok(V::Null)
}

#[allow(non_snake_case)]
pub fn parsePartitionStrategy(strategy: &V, _: &V, _: &V) -> R {
    let s = text(strategy).to_ascii_lowercase();
    match s.as_str() {
        "list" => Ok(V::Int(c::PARTITION_STRATEGY_LIST)),
        "range" => Ok(V::Int(c::PARTITION_STRATEGY_RANGE)),
        "hash" => Ok(V::Int(c::PARTITION_STRATEGY_HASH)),
        _ => Err(format!("unrecognized partitioning strategy \"{}\"", text(strategy))),
    }
}

pub fn preprocess_pubobj_list(list: &V, _: &V) -> R {
    if !list.truthy() {
        return Ok(V::Null);
    }
    if linitial(list).get(f::F_pubobjtype).int() == c::PUBLICATIONOBJ_CONTINUATION {
        return Err("invalid publication object list".to_string());
    }
    let mut previous = c::PUBLICATIONOBJ_CONTINUATION;
    for obj in list.items() {
        if obj.get(f::F_pubobjtype).int() == c::PUBLICATIONOBJ_CONTINUATION {
            obj.set(f::F_pubobjtype, V::Int(previous));
        }
        let kind = obj.get(f::F_pubobjtype).int();
        let table = obj.get(f::F_pubtable);
        let name = obj.get(f::F_name);
        if kind == c::PUBLICATIONOBJ_TABLE {
            if !name.truthy() && !table.truthy() {
                return Err("invalid table name".to_string());
            }
            if name.truthy() {
                let new = V::node(t::T_PublicationTable);
                new.set(f::F_relation, makeRangeVar(&V::Null, &name, &obj.get(f::F_location))?);
                obj.set(f::F_pubtable, new);
                obj.set(f::F_name, V::Null);
            }
        } else if kind == c::PUBLICATIONOBJ_TABLES_IN_SCHEMA || kind == c::PUBLICATIONOBJ_TABLES_IN_CUR_SCHEMA {
            if table.truthy() && table.get(f::F_whereClause).truthy() {
                return Err("WHERE clause not allowed for schema".to_string());
            }
            if table.truthy() && table.get(f::F_columns).truthy() {
                return Err("column specification not allowed for schema".to_string());
            }
            if name.truthy() {
                obj.set(f::F_pubobjtype, V::Int(c::PUBLICATIONOBJ_TABLES_IN_SCHEMA));
            } else if !table.truthy() {
                obj.set(f::F_pubobjtype, V::Int(c::PUBLICATIONOBJ_TABLES_IN_CUR_SCHEMA));
            } else {
                return Err("invalid schema name".to_string());
            }
        }
        previous = obj.get(f::F_pubobjtype).int();
    }
    Ok(V::Null)
}

#[allow(non_snake_case)]
pub fn makeRecursiveViewSelect(relname: &V, aliases: &V, query: &V) -> R {
    let s = V::node(t::T_SelectStmt);
    let w = V::node(t::T_WithClause);
    let cte = V::node(t::T_CommonTableExpr);
    cte.set(f::F_ctename, relname.clone());
    cte.set(f::F_aliascolnames, aliases.clone());
    cte.set(f::F_ctematerialized, V::Int(c::CTEMaterializeDefault));
    cte.set(f::F_ctequery, query.clone());
    cte.set(f::F_location, V::Int(-1));
    w.set(f::F_recursive, V::Int(1));
    w.set(f::F_ctes, list_make1(&cte)?);
    w.set(f::F_location, V::Int(-1));
    let mut targets = V::Null;
    for alias in aliases.items() {
        let rt = V::node(t::T_ResTarget);
        rt.set(f::F_val, makeColumnRef(&alias.get(f::F_sval), &V::Null, &V::Int(-1), &V::Null)?);
        rt.set(f::F_location, V::Int(-1));
        targets = lappend(&targets, &rt)?;
    }
    s.set(f::F_withClause, w);
    s.set(f::F_targetList, targets);
    s.set(f::F_fromClause, list_make1(&makeRangeVar(&V::Null, relname, &V::Int(-1))?)?);
    Ok(s)
}
