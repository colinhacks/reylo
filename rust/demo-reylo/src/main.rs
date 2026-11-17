//! The demo's statements, run. Each `let` names the type the macro gave, so building this crate is
//! the check; running it shows the rows.

mod queries;

use futures_util::TryStreamExt;
use queries::*;
use reylo::Enum;

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

    let rows = post_statuses::Query.run(&pool).await?;
    let status: post_statuses::Status = rows[0].status;
    assert_eq!(status, post_statuses::Status::Draft);
    println!("12. CHECK list  {rows:?} ({})", status.label());

    let post = get_post::Query { id: 1 }.run(&pool).await?.expect("hello");
    let own: PostStatus = post.status;
    assert_eq!(own, PostStatus::Draft);
    println!("13. own enum    {post:?}");

    let mut tx = pool.begin().await?;
    let changed = set_post_status::Query { status: PostStatus::Published, id: 1 }.run(&mut *tx).await?;
    let published = get_post::Query { id: 1 }.run(&mut *tx).await?.expect("hello").status;
    tx.rollback().await?;
    assert_eq!(published, PostStatus::Published);
    println!("14. own param   {changed} changed, now {published:?}");

    let moods = user_moods::Query.run(&pool).await?;
    let mood: Option<Mood> = moods[1].mood;
    assert_eq!((moods[1].id, mood), (2, Some(Mood::Sad)));
    println!("15. own row     {moods:?}");

    // One query's enum as another's, and the crate's as a query's: the same database enum.
    let ada = user_details::Query { id: 1 }.run(&pool).await?.expect("ada").mood.expect("ok");
    let as_param: users_by_mood::Mood = ada.cast();
    let sad: users_by_mood::Mood = Mood::Sad.cast();
    let ok = users_by_mood::Query { mood: as_param }.run(&pool).await?;
    println!("16. cast        {ok:?} {sad:?} ({})", Mood::Ok.label());

    let mut titles = titles_by_author::Query { author: 1 }.stream(&pool);
    let mut streamed = Vec::new();
    while let Some(row) = titles.try_next().await? {
        streamed.push(row.title);
    }
    assert_eq!(streamed.len(), 2);
    println!("17. stream      {streamed:?}");
    println!("18. SQL         {}", get_user::SQL.trim());
    Ok(())
}
