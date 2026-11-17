//! Queries written by `reylo generate rust-sqlx` beside their `.sql` files (src/queries/), and by
//! a build script into OUT_DIR (src/reports/), run. Each `let` names the type the module holds.

mod queries;

mod reports {
    include!(concat!(env!("OUT_DIR"), "/reports.rs"));
}

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").expect("DATABASE_URL names the demo database")).await?;

    let user: Option<queries::get_user::Row> = queries::get_user::Query { id: 1 }.run(&pool).await?;
    let name: &String = &user.as_ref().expect("ada").name;
    println!("1. by key        {user:?} ({name})");

    let n: i64 = queries::users::count_users::Query.run(&pool).await?.n;
    println!("2. count(*)      {n}");

    let rows: Vec<reports::post_counts::Row> = reports::post_counts::Query.run(&pool).await?;
    // Each group has its user, so max(joined) is never NULL.
    let joined: &reylo::sqlx::types::chrono::DateTime<reylo::sqlx::types::chrono::Utc> = &rows[0].joined;
    println!("3. build script  {rows:?} ({joined})");

    // One file declares each enum type once, so one query's value of it is another's parameter.
    let moods = reports::moods::Query.run(&pool).await?;
    let bob: reports::public::Mood = moods[1].mood.expect("bob");
    let same: reports::Mood = bob;
    let sad = reports::sad_users::Query { mood: same }.run(&pool).await?;
    println!("4. shared enum   {moods:?} {sad:?}");
    Ok(())
}
