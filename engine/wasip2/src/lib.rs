//! One database's engine as a WebAssembly component: the native program (engine/native) behind
//! two exports. `call` answers a request the core answers with no database (`ping`, `lex`,
//! `config`), at once. `serve` answers any request the program answers on a line of its standard
//! input, `open`, `run`, `query`, `describe`, `forget` and `close` among them, and returns when the
//! answer is whole: the connections it opens are WASI sockets, which its host carries (src/engine.ts
//! runs it in a worker, src/engine-worker.ts, where a call may wait on the network).
//!
//! The program, its connections and its runtime live as long as the instance. The runtime runs
//! only while `serve` does: before it returns, the tasks the request woke (a connection's own task,
//! told its client is gone) run to their next wait, and a connection's task reads the server's
//! messages when the next request comes, as tokio-postgres's does for a client that sends none.

use std::cell::RefCell;

#[cfg(feature = "postgres")]
type Program = reylo_engine_native::postgres::Postgres;
#[cfg(all(feature = "mysql", not(feature = "postgres")))]
type Program = reylo_engine_native::mysql::Mysql;
#[cfg(all(feature = "sqlite", not(feature = "postgres"), not(feature = "mysql")))]
type Program = reylo_engine_native::sqlite::Sqlite;
#[cfg(all(feature = "duckdb", not(feature = "postgres"), not(feature = "mysql"), not(feature = "sqlite")))]
type Program = reylo_engine_native::duckdb::Duckdb;

wit_bindgen::generate!({
    inline: r#"
        package reylo:engine;

        world engine {
            /// The core's answer to a request that needs no database.
            export call: func(request: string) -> string;
            /// The answer to one request line, with the request's id, as the native program writes it.
            export serve: func(request: string) -> string;
        }
    "#,
});

struct Engine;

/// How many times `serve` yields to the runtime's other tasks after an answer.
const SETTLE: usize = 16;

thread_local! {
    static STATE: RefCell<Option<(tokio::runtime::Runtime, Program)>> = const { RefCell::new(None) };
}

impl Guest for Engine {
    fn call(request: String) -> String {
        let answer = reylo_engine_native::Program::call(&Program::default(), &request);
        // A ping also names the code the component carries besides the engine (build.rs).
        let ping = serde_json::from_str::<serde_json::Value>(&request).is_ok_and(|r| r["op"] == "ping");
        if !ping {
            return answer;
        }
        match serde_json::from_str::<serde_json::Value>(&answer) {
            Ok(mut value) if value["status"] == "ok" => {
                value["host"] = env!("REYLO_ENGINE_HOST_HASH").into();
                value.to_string()
            }
            _ => answer,
        }
    }

    fn serve(request: String) -> String {
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            let (runtime, program) = state.get_or_insert_with(|| (tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a runtime"), Program::default()));
            runtime.block_on(async {
                let answer = reylo_engine_native::answer(program, &request).await;
                // The tasks the request woke run to their next wait before the answer goes back:
                // a connection closed or dropped writes its last message and shuts its socket now,
                // as the native program's runtime, which runs between requests, does it.
                for _ in 0..SETTLE {
                    tokio::task::yield_now().await;
                }
                answer
            })
        })
    }
}

export!(Engine);
