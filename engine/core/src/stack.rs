//! The stack the analysis runs on.
//!
//! A statement's tree is as deep as its expressions nest, and every pass over it recurses: the
//! reading of a parser's tree into Rust (libpg_query's C nodes, `engine/postgres/src/raw.rs`, and
//! serde on each level of SQLite's), the walks of the expression rules, the
//! row-count proof and the JSON shapes, and the drop of the tree. Each parser bounds the depth of
//! what it hands over (libpg_query by the patch in `engine/postgres/vendor/`, SQLite by the limit
//! `engine/sqlite/csrc/reylo_sqlite.c` sets on its expression depth), so the stack all of that
//! needs is bounded too, and every host gives the engine at least that much: natively the analysis
//! runs on a thread of its own with [`STACK_BYTES`] of stack, whatever thread the host calls from
//! (a tokio worker has 2 MiB, a Postgres backend whatever the server was started with); the
//! WebAssembly build is linked with a shadow stack of its own size (`engine/build.sh`). A statement
//! nested past the bound does not parse locally, which every caller already answers soundly: the
//! server's describe alone, any number of rows.

#[cfg(not(target_family = "wasm"))]
use std::cell::Cell;

/// How deeply a statement may nest, in libpg_query's measure, the protobuf messages of its tree,
/// which the engine's reader of the tree counts the same way (`engine/postgres/src/raw.rs`) and
/// refuses past with "parse tree is nested too deeply to read". An
/// expression level is two messages (the `Node` and the node inside it), so this is about 500
/// levels of `a + b + …`, of nested `CASE`, `coalesce` or `json_build_object`, and 165 nested
/// scalar subqueries; a flat list (`a or b or …`, a `CASE` with many `WHEN`s, an `IN` list) does
/// not nest at all. The bound is set in the patch to libpg_query (`engine/postgres/vendor/`), the
/// same for every host, so a statement parses on all of them or on none, and a client engine and
/// `pgreylo` give the same answer. It is what the WebAssembly engine can afford: there the
/// recursion runs on the JavaScript engine's own stack, about 1 MB in Node and shared with the
/// host's frames, and the deepest statement of each shape leaves the host 45% of it
/// (`tests/depth.test.ts`). The
/// other parsers spend more a level, so each has a bound of its own, set where the deepest
/// statement needs no more of Node's stack than that: the MySQL engine bounds `sqlparser`'s
/// recursion to a fifth of this, 200 levels (196 nested function calls, 195 `CASE`s, 97
/// subqueries), and a statement's operators to half (`engine/mysql/src/mysql.rs`); the embedded
/// SQLite reads an expression at most 350 deep, where its own bound is 1,000
/// (`engine/sqlite/csrc/reylo_sqlite.c`).
pub const MAX_DEPTH: usize = 1_000;

/// The stack an analysis runs on natively. At [`MAX_DEPTH`] an unoptimized build needed about
/// 25 MiB while prost decoded libpg_query's tree; this is reserved address space, and only the
/// pages a deep statement touches are ever backed by memory.
pub const STACK_BYTES: usize = 64 << 20;

#[cfg(not(target_family = "wasm"))]
thread_local! {
    static ON_ENGINE_STACK: Cell<bool> = const { Cell::new(false) };
}

/// Run `f` on a stack of [`STACK_BYTES`]: on a thread of the engine's own, or here where this
/// already is one. The thread blocks every signal but those of its own faults, so a signal meant
/// for the host (a Postgres backend's cancel, a terminal's interrupt) is delivered to one of the
/// host's own threads; it starts no process, which would inherit the mask.
#[cfg(not(target_family = "wasm"))]
pub fn run<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    on_stack(true, f)
}

/// Run a host's own work on a stack of [`STACK_BYTES`], so that the engine's calls it makes run
/// there, and its own walks over what they return have the room too (Reylo Framework runs every
/// command this way). Unlike [`run`] it leaves the signal mask as it is: the host's work starts
/// threads and processes, which inherit the mask, and handles its own signals.
#[cfg(not(target_family = "wasm"))]
pub fn host<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    on_stack(false, f)
}

#[cfg(not(target_family = "wasm"))]
fn on_stack<T: Send>(block_signals: bool, f: impl FnOnce() -> T + Send) -> T {
    if ON_ENGINE_STACK.with(Cell::get) {
        return f();
    }
    let mut job = Some(f);
    let done = std::thread::scope(|scope| {
        let spawned = {
            let _blocked = block_signals.then(Signals::block);
            std::thread::Builder::new().name("reylo-engine".into()).stack_size(STACK_BYTES).spawn_scoped(scope, || {
                ON_ENGINE_STACK.with(|on| on.set(true));
                job.take().map(|f| f())
            })
        };
        match spawned {
            Ok(thread) => thread.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic)),
            Err(_) => None,
        }
    });
    match (done, job) {
        (Some(value), _) => value,
        // No thread to be had (the process is out of threads or memory): the analysis runs where
        // it was called, as it did before it had a thread of its own, and only a statement deep
        // enough to need the room is then the host's stack's to bear.
        (None, Some(f)) => f(),
        (None, None) => unreachable!("the engine's thread neither ran nor left its work"),
    }
}

/// The WebAssembly build has one thread, and its stack is the size it was linked with.
#[cfg(target_family = "wasm")]
pub fn run<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    f()
}

/// See [`run`].
#[cfg(target_family = "wasm")]
pub fn host<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    f()
}

/// While alive, every signal is blocked on this thread, and a thread started meanwhile starts
/// with them blocked; the old mask comes back on drop.
#[cfg(not(target_family = "wasm"))]
struct Signals {
    #[cfg(unix)]
    old: libc::sigset_t,
}

#[cfg(not(target_family = "wasm"))]
impl Signals {
    fn block() -> Signals {
        #[cfg(unix)]
        unsafe {
            let mut all: libc::sigset_t = std::mem::zeroed();
            let mut old: libc::sigset_t = std::mem::zeroed();
            libc::sigfillset(&mut all);
            // Not the signals a fault raises on this thread itself: blocked, a fault (the guard
            // page of an overflowed stack among them) is raised again and again, and the process
            // spins where it should have stopped.
            for fault in [libc::SIGSEGV, libc::SIGBUS, libc::SIGILL, libc::SIGFPE, libc::SIGTRAP, libc::SIGABRT, libc::SIGSYS] {
                libc::sigdelset(&mut all, fault);
            }
            libc::pthread_sigmask(libc::SIG_SETMASK, &all, &mut old);
            Signals { old }
        }
        #[cfg(not(unix))]
        Signals {}
    }
}

#[cfg(not(target_family = "wasm"))]
impl Drop for Signals {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            libc::pthread_sigmask(libc::SIG_SETMASK, &self.old, std::ptr::null_mut());
        }
    }
}

/// Statements that nest `n` levels deep, the shapes deep SQL takes in practice, for the depth tests
/// of every database's engine; each reads `users` at its top level.
#[doc(hidden)]
pub fn nested(shape: &str, n: usize) -> String {
    let fold = |start: &str, f: &dyn Fn(String, usize) -> String| (0..n).fold(start.to_string(), |s, i| f(s, i));
    match shape {
        "sum" => format!("select {} as x from users", vec!["id"; n].join(" + ")),
        "concat" => format!("select {} as x from users", vec!["name"; n].join(" || ")),
        "and_or" => format!("select id from users where {}", fold("true", &|s, i| format!("id = {i} {} ({s})", if i % 2 == 0 { "or" } else { "and" }))),
        "case" => format!("select {} as x from users", fold("0", &|s, i| format!("case when id = {i} then {i} else {s} end"))),
        "coalesce" => format!("select {} as x from users", fold("nick", &|s, _| format!("coalesce({s}, name)"))),
        "json" => format!("select {} as x from users", fold("id", &|s, _| format!("json_build_object('a', {s})"))),
        "subquery" => format!("select ({}) as x from users", fold("select 1", &|s, _| format!("select ({s})"))),
        // Flat: neither nests.
        "or" => format!("select id from users where {}", (0..n).map(|i| format!("id = {i}")).collect::<Vec<_>>().join(" or ")),
        "parens" => format!("select {}id{} as x from users", "(".repeat(n), ")".repeat(n)),
        _ => unreachable!("no shape {shape}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_engine_runs_on_a_thread_of_its_own_once() {
        let outer = run(|| (std::thread::current().id(), std::thread::current().name().map(String::from), run(|| std::thread::current().id())));
        assert_eq!(outer.1.as_deref(), Some("reylo-engine"));
        // A call from the engine's own thread stays on it.
        assert_eq!(outer.0, outer.2);
        assert_ne!(outer.0, std::thread::current().id());
    }

    #[cfg(unix)]
    #[test]
    fn the_engine_thread_takes_no_signal_meant_for_the_host_and_the_caller_keeps_its_own() {
        unsafe fn mask() -> libc::sigset_t {
            let mut set: libc::sigset_t = std::mem::zeroed();
            libc::pthread_sigmask(libc::SIG_SETMASK, std::ptr::null(), &mut set);
            set
        }
        let before = unsafe { mask() };
        let inside = run(|| unsafe {
            let set = mask();
            [libc::SIGINT, libc::SIGTERM, libc::SIGUSR1, libc::SIGSEGV, libc::SIGBUS].map(|s| libc::sigismember(&set, s) == 1)
        });
        assert_eq!(inside, [true, true, true, false, false]);
        // A host's own work keeps the mask it had: a process it starts must take SIGTERM.
        let hosted = host(|| unsafe { libc::sigismember(&mask(), libc::SIGTERM) == 1 });
        assert_eq!(hosted, unsafe { libc::sigismember(&before, libc::SIGTERM) == 1 });
        let after = unsafe { mask() };
        assert_eq!(unsafe { libc::sigismember(&before, libc::SIGINT) }, unsafe { libc::sigismember(&after, libc::SIGINT) });
    }
}
