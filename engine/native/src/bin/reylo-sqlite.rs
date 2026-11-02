//! SQLite's engine alone as a native program, as `reylo engine sqlite` serves it: `reylo-sqlite`
//! serves requests line by line, and `reylo-sqlite call` answers the one request on standard input.
fn main() {
    reylo_engine_native::main(reylo_engine_native::sqlite::Sqlite::default());
}
