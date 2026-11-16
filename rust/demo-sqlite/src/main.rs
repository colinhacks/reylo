//! The demo's statements on SQLite, typed by Reylo's engine at compile time from `demo.db`, which
//! `schema.sql` makes. Each `let` names the type the macro gave, so building this crate is the
//! check; running it shows the rows.

#[reylo::query("select u.name, p.title from users u left join posts p on p.author_id = u.id order by u.name, p.id")]
mod names_and_titles {}

#[reylo::query("select count(*) as n from users")]
mod count_users {}

#[reylo::query("select coalesce(nickname, name) as display from users order by id")]
mod display_names {}

#[reylo::query("select title from posts where author_id = :author order by id")]
mod titles_by_author {}

#[reylo::query("select id, name, nickname from users where id = :id")]
mod get_user {}

#[reylo::query(
    "select json_object('id', u.id, 'name', u.name, 'titles', (select json_group_array(p.title) from posts p where p.author_id = u.id)) as profile from users u where u.id = :id"
)]
mod get_profile {}

#[reylo::query("select score, active, visits, avatar, nickname is null as anonymous from users order by id")]
mod user_details {}

#[reylo::query("insert into posts (author_id, title) values (:author, :title) returning id, title")]
mod add_post {}

#[reylo::query("update users set nickname = :nickname where id = :id")]
mod set_nickname {}

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    // The file the macro read at compile time, wherever the demo is run from.
    let pool = sqlx::SqlitePool::connect(concat!("sqlite:", env!("CARGO_MANIFEST_DIR"), "/demo.db")).await?;

    let rows = names_and_titles::Query.run(&pool).await?;
    let title: &Option<String> = &rows[rows.len() - 1].title;
    println!("1. left join    {rows:?} (bob's title: {title:?})");

    let n: i64 = count_users::Query.run(&pool).await?.n;
    println!("2. count(*)     {n}");

    let rows = display_names::Query.run(&pool).await?;
    let display: &String = &rows[1].display;
    println!("3. coalesce     {rows:?} ({display})");

    let rows = titles_by_author::Query { author: 1 }.run(&pool).await?;
    println!("4. :author      {rows:?}");

    let user: Option<get_user::Row> = get_user::Query { id: 2 }.run(&pool).await?;
    println!("5. by key       {user:?}");

    let profile = get_profile::Query { id: 1 }.run(&pool).await?.expect("ada").profile.0;
    let (id, name): (i64, &String) = (profile.id, &profile.name);
    println!("6. json         {profile:?} ({id}, {name})");

    let rows = user_details::Query.run(&pool).await?;
    let (score, active, visits, avatar, anonymous): (f64, bool, i64, &Option<Vec<u8>>, bool) = (rows[0].score, rows[0].active, rows[0].visits, &rows[0].avatar, rows[0].anonymous);
    println!("7. types        {score} {active} {visits} {avatar:?} {anonymous}");

    // Writes, in a transaction that is rolled back: one returns its row, the other a count.
    let mut tx = pool.begin().await?;
    let post = add_post::Query { author: 2, title: "first" }.run(&mut *tx).await?;
    let renamed: u64 = set_nickname::Query { nickname: Some("b"), id: 2 }.run(&mut *tx).await?;
    tx.rollback().await?;
    println!("8. writes       {post:?}, {renamed} renamed");
    Ok(())
}
