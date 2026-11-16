//! The demo's statements, run. Each `let` names the type the macro gave, so building this crate is
//! the check; running it shows the rows.

mod queries;

use queries::*;

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").expect("DATABASE_URL names the demo database")).await?;

    let rows = names_and_titles::Query.run(&pool).await?;
    let title: &Option<String> = &rows[rows.len() - 1].title;
    println!("1. left join    {rows:?} (bob's title: {title:?})");

    let row = count_users::Query.run(&pool).await?;
    let n: i64 = row.n;
    println!("2. count(*)     {n}");

    let rows = display_names::Query.run(&pool).await?;
    let display: &String = &rows[1].display;
    println!("3. coalesce     {rows:?} ({display})");

    let rows = titles_by_author::Query { author: 1 }.run(&pool).await?;
    println!("4. :author      {rows:?}");

    let user: Option<get_user::Row> = get_user::Query { id: 2 }.run(&pool).await?;
    println!("5. by key       {user:?} ({})", user.as_ref().map_or("none", |u| u.display_name()));

    let profile = get_profile::Query { id: 1 }.run(&pool).await?.expect("ada").profile.0;
    let (id, name): (i32, &String) = (profile.id, &profile.name);
    println!("6. jsonb        {profile:?} ({id}, {name})");

    let rows = post_tags::Query.run(&pool).await?;
    let tags: &Vec<Option<String>> = &rows[1].tags;
    println!("7. text[]       {rows:?} ({tags:?})");

    let row = user_details::Query { id: 1 }.run(&pool).await?.expect("ada");
    let since: reylo::sqlx::postgres::types::PgInterval = row.since;
    let (joined, token, balance): (reylo::sqlx::types::chrono::DateTime<reylo::sqlx::types::chrono::Utc>, reylo::sqlx::types::Uuid, reylo::sqlx::types::Decimal) = (row.joined, row.token, row.balance);
    println!("8. types        mood {:?} ({}), joined {joined}, token {token}, balance {balance}, since {since:?}", row.mood, row.mood.map_or("none", |m| m.label()));

    let sad = users_by_mood::Query { mood: users_by_mood::Mood::Sad }.run(&pool).await?;
    println!("9. :mood        {sad:?}");

    let page = user_page(&pool, 1).await?.expect("ada");
    println!("10. composed    {} {}", page.user, serde_json::to_string(&page).expect("json"));

    let mut tx = pool.begin().await?;
    let changed: u64 = set_nickname::Query { nickname: Some("b"), id: 2 }.run(&mut *tx).await?;
    tx.rollback().await?;
    println!("11. write       {changed} changed");
    Ok(())
}
