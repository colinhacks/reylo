//! Postgres's engine alone as a native program, as `reylo engine postgres` serves it: `reylo-postgres`
//! serves requests line by line, and `reylo-postgres call` answers the one request on standard input.
fn main() {
    reylo_engine_native::main(reylo_engine_native::postgres::Postgres::default());
}
