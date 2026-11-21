fn main() {
    let sql = std::env::args().nth(1).unwrap();
    let p = syntax::postgres::parse(&sql, 0).unwrap();
    println!("{}", serde_json::to_string(&p.protobuf).unwrap());
}
