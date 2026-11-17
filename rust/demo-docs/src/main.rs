//! The docs' Rust snippets (site/content/docs/index.mdx, rust.mdx, rust-usage.mdx), each as the page
//! writes it, with the type the page names for each value written out, run against the schema of
//! the docs' examples.

use futures_util::TryStreamExt;
use reylo::Enum;
use sqlx::PgPool;

// The intro (index.mdx).
#[reylo::query("
    select u.id, u.name, count(p.id)::int as post_count
    from users u
    left join posts p on p.user_id = u.id
    where u.id = :id::int
    group by u.id
")]
pub mod get_author {}

// Setup (rust.mdx), and The module (rust-usage.mdx).
#[reylo::query("select id, name, email from users where id = :id::int")]
pub mod get_user {}

// .run() (rust-usage.mdx).
#[reylo::query("select count(*) as n from users")]
pub mod count_users {}

// .stream().
#[reylo::query("select id, title from posts where user_id = :user_id::int order by id")]
pub mod posts_by_user {}

#[reylo::query("update users set name = :name where id = :id::int")]
pub mod rename_user {}

// Parameters.
#[reylo::query("select id, name from users where role = :role order by id")]
pub mod users_by_role {}

#[reylo::query("select id, title from posts where user_id = coalesce(:user_id::nullable(int), user_id) order by id")]
pub mod posts_of {}

// Enums.
#[reylo::query("select id, status from posts order by id")]
pub mod post_statuses {}

#[reylo::query("select id, role from users where id = :id::int")]
pub mod get_role {}

// Your own types.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Role { Admin, Member }

#[reylo::query("select id, name, role from users where id = :id::int")]
pub mod get_member {
    pub struct Row {
        pub role: crate::Role,
    }
}

#[reylo::enum_type("public.user_role")]
#[derive(Debug, PartialEq)]
pub enum UserRole {} // filled in: Admin, Member

pub struct Member { pub id: i32, pub name: String, pub role: UserRole }

#[reylo::query("select id, name, role from users where id = :id::int")]
pub mod find_member {
    pub use crate::Member as Row;
}

// Your own methods.
#[reylo::query("select id, name, email from users where id = :id::int")]
pub mod get_contact {
    impl Row {
        pub fn label(&self) -> &str {
            self.email.as_deref().unwrap_or(&self.name)
        }
    }
}

// JSON.
#[reylo::query("select json_build_object('id', u.id, 'name', u.name) as author from users u where u.id = :id::int")]
pub mod author_json {}

// Dates and times.
#[reylo::query("select title, published_at from posts order by id")]
pub mod published {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = PgPool::connect(&std::env::var("DATABASE_URL")?).await?;

    // author: Option<get_author::Row>
    let author: Option<get_author::Row> = get_author::Query { id: 1 }.run(&pool).await?;
    let a = author.expect("ada");
    let (_, _, _): (i32, String, i32) = (a.id, a.name.clone(), a.post_count);
    println!("intro       {a:?}");

    // user: Option<get_user::Row>
    let user = get_user::Query { id: 1 }.run(&pool).await?;
    let u: get_user::Row = user.expect("ada");
    let (_, _, _): (i32, String, Option<String>) = (u.id, u.name.clone(), u.email.clone());
    println!("setup       {u:?}");

    let count: count_users::Row = count_users::Query.run(&pool).await?;
    let user: Option<get_user::Row> = get_user::Query { id: 1 }.run(&pool).await?;
    let posts: Vec<posts_by_user::Row> =
        posts_by_user::Query { user_id: 1 }.run(&pool).await?;
    let changed: u64 = rename_user::Query { name: "ada", id: 1 }.run(&pool).await?;
    let _: i64 = count.n;
    println!("run         {count:?} {} {} {changed}", user.is_some(), posts.len());

    let mut tx = pool.begin().await?;
    let user: Option<get_user::Row> = get_user::Query { id: 1 }.run(&mut *tx).await?;
    tx.commit().await?;
    println!("transaction {}", user.is_some());

    let role = users_by_role::UserRole::Admin;
    let admins = users_by_role::Query { role }.run(&pool).await?;
    let everyone = posts_of::Query { user_id: None }.run(&pool).await?;
    let ada_s = posts_of::Query { user_id: Some(1) }.run(&pool).await?;
    println!("params      {admins:?} {} {}", everyone.len(), ada_s.len());

    let mut posts = posts_by_user::Query { user_id: 1 }.stream(&pool);
    let mut titles = Vec::new();
    while let Some(post) = posts.try_next().await? {
        titles.push(post.title);
    }
    println!("stream      {titles:?}");

    let label: &str = users_by_role::UserRole::Admin.label();
    let rows = post_statuses::Query.run(&pool).await?;
    let status: post_statuses::Status = rows[0].status;
    let ada = get_role::Query { id: 1 }.run(&pool).await?.expect("ada");
    let admins = users_by_role::Query { role: ada.role.cast() }.run(&pool).await?;
    println!("enums       {label} {status:?} {admins:?}");

    let member = get_member::Query { id: 2 }.run(&pool).await?.expect("bob");
    let role: Role = member.role;
    let found = find_member::Query { id: 1 }.run(&pool).await?.expect("ada");
    let (_, _, whole): (i32, String, UserRole) = (found.id, found.name, found.role);
    assert_eq!((role, whole), (Role::Member, UserRole::Admin));
    println!("own types   {role:?} {whole:?}");

    let contact = get_contact::Query { id: 2 }.run(&pool).await?.expect("bob");
    println!("methods     {}", contact.label());

    let row = author_json::Query { id: 1 }.run(&pool).await?.expect("ada");
    let author: author_json::Author = row.author.0;
    let (_, _): (i32, String) = (author.id, author.name.clone());
    println!("json        {author:?}");

    let rows = published::Query.run(&pool).await?;
    let _: Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>> = rows[0].published_at;
    println!("dates       {rows:?}");

    // Migrations (rust.mdx), on a database of their own, whose `reylo/migrations` are the demo's.
    let pool = PgPool::connect(&std::env::var("MIGRATE_URL")?).await?;
    // ran: Vec<String>, the names of the migrations it ran
    let ran = reylo::migrate!().run(&pool).await?;
    let _: &Vec<String> = &ran;
    let status = reylo::migrate!().status(&pool).await?;
    if !status.is_current() {
        eprintln!("{status}");
    }
    assert!(status.is_current() && ran == ["20260101000000_notes.sql"]);
    println!("migrations  {ran:?}");
    Ok(())
}
