//! `@reylosql/sqlite`'s engine as a native program: `reylo-sqlite` serves requests line by line, and
//! `reylo-sqlite call` answers the one request on standard input.
fn main() {
    reylo_engine_native::main(reylo_engine_native::sqlite::Sqlite::default());
}
