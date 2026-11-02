//! DuckDB's engine alone as a native program, as `reylo engine duckdb` serves it: `reylo-duckdb`
//! serves requests line by line, and `reylo-duckdb call` answers the one request on standard input.
fn main() {
    reylo_engine_native::main(reylo_engine_native::duckdb::Duckdb);
}
