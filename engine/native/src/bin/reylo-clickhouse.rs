//! ClickHouse's engine alone as a native program, as `reylo engine clickhouse` serves it:
//! `reylo-clickhouse` serves requests line by line, and `reylo-clickhouse call` answers the one
//! request on standard input.
fn main() {
    reylo_engine::main(reylo_engine::clickhouse::Clickhouse::default());
}
