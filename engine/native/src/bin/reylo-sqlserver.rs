//! SQL Server's engine alone as a native program, as `reylo engine sqlserver` serves it:
//! `reylo-sqlserver` serves requests line by line, and `reylo-sqlserver call` answers the one request
//! on standard input.
fn main() {
    reylo_engine_native::main(reylo_engine_native::sqlserver::Sqlserver::default());
}
