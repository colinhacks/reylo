//! The demo's statements, each typed by Reylo's engine at compile time. Each attribute fills its
//! module with `Params` (the statement's parameters, with `run`) and `Row` (one row of its result).

use sqlx::PgPool;

/// A left join under `order by`: bob has no posts, so his title is NULL.
#[reylo::query("select u.name, p.title from users u left join posts p on p.author_id = u.id order by u.name, p.id")]
pub mod names_and_titles {}

/// `count(*)`: exactly one row, never NULL.
#[reylo::query("select count(*) as n from users")]
pub mod count_users {}

/// `coalesce` of a nullable column and a NOT NULL one: never NULL.
#[reylo::query("select coalesce(nickname, name) as display from users order by id")]
pub mod display_names {}

#[reylo::query("select title from posts where author_id = :author order by id")]
pub mod titles_by_author {}

// An equality on the primary key: at most one row. The crate's own methods sit with the query.
#[reylo::query(r#"
    -- One user, by id.
    select id, name, nickname
    from users
    where id = :id
"#)]
pub mod get_user {
    impl Row {
        pub fn display_name(&self) -> &str {
            self.nickname.as_deref().unwrap_or(&self.name)
        }
    }
}

/// JSON the statement builds, typed field by field.
#[reylo::query(
    "select jsonb_build_object('id', u.id, 'name', u.name, 'titles', (select coalesce(jsonb_agg(p.title order by p.id), '[]') from posts p where p.author_id = u.id)) as profile from users u where u.id = :id"
)]
pub mod get_profile {}

/// An array column: Postgres lets any element be NULL, so each is an Option.
#[reylo::query("select tags from posts order by id")]
pub mod post_tags {}

/// An enum, a timestamptz, a uuid, a numeric and an interval, each its Rust type.
#[reylo::query("select mood, joined, token, balance, joined - joined as since from users where id = :id")]
pub mod user_details {}

/// An enum parameter, of the enum type the module holds.
#[reylo::query("select name from users where mood = :mood order by id")]
pub mod users_by_mood {}

/// A write with no `returning`: `run` gives how many rows changed.
#[reylo::query("update users set nickname = :nickname where id = :id")]
pub mod set_nickname {}

// A trait of the crate's own on a generated row, from outside its module.
impl std::fmt::Display for get_user::Row {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{} {}", self.id, self.display_name())
    }
}

// Rows held in the crate's own struct, and a function whose signature names them.
#[derive(Debug, serde::Serialize)]
pub struct UserPage {
    pub user: get_user::Row,
    pub titles: Vec<titles_by_author::Row>,
}

pub async fn user_page(pool: &PgPool, id: i32) -> sqlx::Result<Option<UserPage>> {
    let Some(user) = (get_user::Params { id }).run(pool).await? else { return Ok(None) };
    let titles = titles_by_author::Query { author: id }.run(pool).await?;
    Ok(Some(UserPage { user, titles }))
}
