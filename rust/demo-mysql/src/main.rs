//! The demo's statements on MySQL, typed by Reylo's engine at compile time from the database
//! `DATABASE_URL` names, which `schema.sql` fills. Each `let` names the type the macro gave, so
//! building this crate is the check; running it shows the rows.

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
    "select json_object('id', u.id, 'name', u.name, 'titles', (select json_arrayagg(p.title) from posts p where p.author_id = u.id)) as profile from users u where u.id = :id"
)]
mod get_profile {}

/// MySQL's own types: BOOLEAN, ENUM, TIMESTAMP, DATE, TIME, DECIMAL, YEAR, BIT and BLOB.
#[reylo::query("select active, status, joined, born, span, balance, since, flags, avatar from users where id = 1")]
mod user_details {}

/// An ENUM parameter: the enum of its values.
#[reylo::query("select name from users where status = :status order by id")]
mod users_by_status {}

#[reylo::query("insert into posts (author_id, title) values (:author, :title)")]
mod add_post {}

#[reylo::query("update users set nickname = :nickname where id = :id")]
mod set_nickname {}

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    let pool = sqlx::MySqlPool::connect(&std::env::var("DATABASE_URL").expect("DATABASE_URL names the demo database")).await?;

    let rows = names_and_titles::Query.run(&pool).await?;
    let title: &Option<String> = &rows[rows.len() - 1].title;
    println!("1. left join    {rows:?} (bob's title: {title:?})");

    let n: i64 = count_users::Query.run(&pool).await?.n;
    println!("2. count(*)     {n}");

    let rows = display_names::Query.run(&pool).await?;
    let display: &String = &rows[1].display;
    println!("3. coalesce     {rows:?} ({display})");

    // The parameter takes the type of the column it is compared with: an unsigned BIGINT.
    let rows = titles_by_author::Query { author: 1 }.run(&pool).await?;
    println!("4. :author      {rows:?}");

    let user = get_user::Query { id: 2 }.run(&pool).await?;
    let (id, nickname): (Option<u64>, Option<String>) = (user.as_ref().map(|u| u.id), user.clone().and_then(|u| u.nickname));
    println!("5. by key       {user:?} ({id:?}, nickname: {nickname:?})");

    let profile = get_profile::Query { id: 1 }.run(&pool).await?.expect("ada").profile.0;
    let (id, name): (u64, &String) = (profile.id, &profile.name);
    println!("6. json         {profile:?} ({id}, {name})");

    let ada = user_details::Query.run(&pool).await?.expect("ada");
    let (active, status, born, since, flags): (bool, user_details::Status, Option<reylo::sqlx::types::chrono::NaiveDate>, Option<u16>, Option<u64>) = (ada.active, ada.status, ada.born, ada.since, ada.flags);
    let joined: reylo::sqlx::types::chrono::DateTime<reylo::sqlx::types::chrono::Utc> = ada.joined;
    let span: Option<reylo::sqlx::mysql::types::MySqlTime> = ada.span;
    let balance: reylo::sqlx::types::Decimal = ada.balance;
    println!("7. types        {active} {status:?} {joined} {born:?} {span:?} {balance} {since:?} {flags:?} {:?}", ada.avatar);

    // One query's ENUM value as another's: the same values, so `cast` compiles.
    let new = users_by_status::Query { status: reylo::Enum::cast(status) }.run(&pool).await?;
    assert_eq!(status, user_details::Status::New);
    println!("8. :status      {new:?}");

    // Writes, in a transaction that is rolled back: each returns how many rows it wrote.
    let mut tx = pool.begin().await?;
    let added: u64 = add_post::Query { author: 2, title: "first" }.run(&mut *tx).await?;
    let renamed: u64 = set_nickname::Query { nickname: Some("b"), id: 2 }.run(&mut *tx).await?;
    tx.rollback().await?;
    println!("9. writes       {added} added, {renamed} renamed");
    Ok(())
}
