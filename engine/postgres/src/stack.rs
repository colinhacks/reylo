//! libpg_query on the engine's stack ([`reylo_engine_core::stack`]), and how deeply a statement it
//! hands over may nest.

pub use crate::raw::Parsed;
pub use reylo_engine_core::stack::{host, nested, run, MAX_DEPTH, STACK_BYTES};

/// Whether a parse error is libpg_query's refusal of a tree deeper than [`MAX_DEPTH`], or of one
/// its stack check stopped (which the engine's stack leaves to a host that could not start a
/// thread for it).
pub fn too_deep(error: &str) -> bool {
    error.contains("nested too deeply") || error.contains("stack depth limit exceeded")
}

/// Parse `sql` with libpg_query, on the engine's stack. Every parse of the engine and of Reylo
/// Framework comes through here: the reading of the tree is the deepest recursion of an analysis,
/// and a caller that is not on the engine's stack (a framework helper on a tokio worker) gets the
/// room for it all the same. The tree comes back to the caller, whose own walks over it take far
/// less stack per level.
///
/// An analysis reads one statement, and each probe of it, many times over, and a host replays it
/// round after round, so the trees of the last statements parsed are kept by their text: a copy
/// of one costs a fraction of the parse.
pub fn parse(sql: &str) -> pg_query::Result<Parsed> {
    if let Some(tree) = PARSED.with(|kept| kept.borrow().iter().find(|(text, _)| text == sql).map(|(_, tree)| tree.clone())) {
        return Ok(tree);
    }
    let tree = run(|| crate::raw::parse(sql))?;
    PARSED.with(|kept| {
        let mut kept = kept.borrow_mut();
        if kept.len() >= KEPT {
            kept.remove(0);
        }
        kept.push((sql.to_string(), tree.clone()));
    });
    Ok(tree)
}

/// What `read` answers for `sql`, kept with the last answers by the text: a pure reading of a
/// statement, which an analysis asks for over and over ([`parse`]).
pub(crate) fn kept<T: Clone + 'static>(answers: &'static std::thread::LocalKey<std::cell::RefCell<Vec<(String, T)>>>, sql: &str, read: impl FnOnce() -> T) -> T {
    if let Some(answer) = answers.with(|kept| kept.borrow().iter().find(|(text, _)| text == sql).map(|(_, answer)| answer.clone())) {
        return answer;
    }
    let answer = read();
    answers.with(|kept| {
        let mut kept = kept.borrow_mut();
        if kept.len() >= KEPT {
            kept.remove(0);
        }
        kept.push((sql.to_string(), answer.clone()));
    });
    answer
}

/// How many trees [`parse`] keeps: more than the statements one analysis parses, its probes and
/// the expressions it reads alone included.
const KEPT: usize = 64;

thread_local! {
    static PARSED: std::cell::RefCell<Vec<(String, Parsed)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Print a tree back to SQL with libpg_query, on the engine's stack (see [`parse`]).
pub fn deparse(tree: &pg_query::protobuf::ParseResult) -> pg_query::Result<String> {
    run(|| crate::raw::deparse(tree))
}

/// The deepest statement of each shape that parses under [`MAX_DEPTH`].
#[cfg(test)]
pub(crate) const BOUNDS: &[(&str, usize)] = &[("sum", 496), ("concat", 496), ("and_or", 495), ("case", 493), ("coalesce", 495), ("json", 495), ("subquery", 164)];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze::{analyze, AnalyzeError};

    /// On a thread with a small stack, as a host may call from, so that the engine's own stack is
    /// what carries the parse.
    fn on_small_stack<T: Send>(f: impl FnOnce() -> T + Send) -> T {
        std::thread::scope(|s| std::thread::Builder::new().stack_size(256 << 10).spawn_scoped(s, f).unwrap().join().unwrap())
    }

    #[test]
    fn every_shape_parses_at_the_bound_and_none_one_level_past_it() {
        on_small_stack(|| {
            for &(shape, n) in BOUNDS {
                assert!(analyze(&nested(shape, n)).is_ok(), "{shape} at {n}");
                assert_eq!(analyze(&nested(shape, n + 1)).err(), Some(AnalyzeError::TooDeep), "{shape} at {}", n + 1);
            }
        });
        // Far past it libpg_query's stack check stops the parse before the bound does, as the same
        // refusal; and what does not nest is no deeper for being long.
        assert_eq!(analyze(&nested("sum", 100_000)).err(), Some(AnalyzeError::TooDeep));
        assert!(analyze(&nested("or", 10_000)).is_ok());
        assert!(analyze(&nested("parens", 5_000)).is_ok());
        assert!(AnalyzeError::TooDeep.to_string().contains("1000 levels of its parse tree"));
    }

    #[test]
    fn a_statement_past_the_bound_is_refused_where_its_tree_is_read() {
        let error = parse(&nested("sum", 497)).err().map(|e| e.to_string()).unwrap_or_default();
        assert!(error.contains(crate::raw::TOO_DEEP), "{error}");
        assert!(too_deep(&error));
        assert!(!too_deep("syntax error at or near \"selec\""));
    }
}
