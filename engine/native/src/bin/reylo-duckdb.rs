//! `@reylosql/duckdb`'s engine as a native program: `reylo-duckdb` serves requests line by line, and
//! `reylo-duckdb call` answers the one request on standard input.
fn main() {
    reylo_engine_native::main(reylo_engine_native::duckdb::Duckdb);
}
