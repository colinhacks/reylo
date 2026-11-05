// Compiled against Postgres with DATABASE_URL set: does query! accept a null for a parameter compared with a primary key?
pub async fn check(pool: &sqlx::PgPool) {
    let _ = sqlx::query!("select name from users where id = $1", None::<i32>).fetch_all(pool).await;
}
fn main() {}
