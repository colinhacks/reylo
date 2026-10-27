//! `@reylosql/mysql`'s engine as a native program: `reylo-mysql` serves requests line by line, and
//! `reylo-mysql call` answers the one request on standard input.
fn main() {
    reylo_engine_native::main(reylo_engine_native::mysql::Mysql::default());
}
