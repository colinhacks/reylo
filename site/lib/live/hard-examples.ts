// The playground's own examples, after the main page's Inference examples and under Advanced in its menu (lib/live/app.tsx): SQL that a type checker is expected to give up on, each with the payoff of its types in a comment. They are on /play alone, never on the main page. The build types each through the same pipeline as the Inference examples (lib/live/play-data.ts), against the main page's Postgres schema, and fails when one no longer types without a diagnostic or takes a slug an Inference example has.
import type { InferenceStatement } from './inference.ts';

/** The menu's group for these examples. */
export const ADVANCED = 'Advanced';

export const HARD_EXAMPLES: InferenceStatement[] = [
  {
    title: 'Window functions',
    slug: 'window-functions',
    sql: `-- lag() is null on the first row
select id, title,
  row_number() over (
    partition by user_id
    order by published_at
  ) as n,
  lag(title) over (order by id) as prev
from posts`,
  },
  {
    title: 'Recursive queries',
    slug: 'recursive-queries',
    sql: `-- the numbers 1 to 10
with recursive n(i) as (
  select 1
  union all
  select i + 1 from n where i < 10
)
select i from n`,
  },
  {
    title: 'Lateral joins',
    slug: 'lateral-joins',
    sql: `-- each user's latest post, or null for none
select u.name, p.title, p.published_at
from users u
left join lateral (
  select title, published_at
  from posts
  where user_id = u.id
  order by published_at desc nulls last
  limit 1
) p on true`,
  },
  {
    title: 'DISTINCT ON',
    slug: 'distinct-on',
    sql: `-- one row per author: their latest post
select distinct on (user_id)
  user_id, title, published_at
from posts
order by user_id, published_at desc nulls last`,
  },
  {
    title: 'Filtered aggregates',
    slug: 'filtered-aggregates',
    sql: `-- no group by: exactly one row
select
  count(*) filter (where published_at is null)::int as drafts,
  count(*) filter (where published_at is not null)::int as published,
  -- null when there are no posts
  max(published_at) as latest
from posts`,
  },
  {
    title: 'Rollups',
    slug: 'rollups',
    sql: `-- the grand total's row has a null user_id
select user_id, count(*)::int as posts
from posts
group by rollup (user_id)
order by user_id`,
  },
  {
    title: 'CASE expressions',
    slug: 'case-expressions',
    sql: `select id,
  -- with an else: never null
  case
    when published_at is null then 'draft'
    when published_at > now() then 'scheduled'
    else 'published'
  end as status,
  -- no else: null when nothing matches
  case when body is null then 'empty' end as note
from posts`,
  },
  {
    title: 'Unnesting arrays',
    slug: 'unnesting-arrays',
    sql: `-- one row per tag; a text[] element may be null
select u.name, t.tag
from users u
cross join unnest(u.tags) as t(tag)`,
  },
  {
    title: 'Date series',
    slug: 'date-series',
    sql: `-- one row per day of the last week
select d::date as day
from generate_series(
  current_date - 6,
  current_date,
  interval '1 day'
) as d`,
  },
  {
    title: 'VALUES lists',
    slug: 'values-lists',
    sql: `-- a lookup table written inline
select u.name, r.label
from users u
join (values
  ('admin', 'Administrator'),
  ('member', 'Member')
) as r(role, label) on r.role = u.role::text`,
  },
  {
    title: 'JSON operators',
    slug: 'json-operators',
    sql: `-- typed by payload's JSON Schema check;
-- a field it doesn't require may be null
select id,
  payload -> 'x' as x,
  payload -> 'y' as y,
  payload ->> 'code' as code
from events
where kind = 'click'`,
  },
  {
    title: 'Whole rows as JSON',
    slug: 'whole-rows-as-json',
    sql: `-- every column keeps its type;
-- null for a user with no posts
select u.name,
  (
    select jsonb_agg(p order by p.id)
    from posts p
    where p.user_id = u.id
  ) as posts
from users u`,
  },
  {
    title: 'Upserts',
    slug: 'upserts',
    sql: `-- on conflict do update: always one row
insert into users (name, email)
values (:name, :email::text)
on conflict (email) do update
  set name = excluded.name
returning id, name`,
  },
  {
    title: 'Old and new values',
    slug: 'old-and-new-values',
    sql: `-- the row before and after the update (Postgres 18);
-- a primary key: at most one row
update users
set name = :name
where id = :id
returning old.name as old_name, new.name as new_name`,
  },
  {
    title: 'Writable CTEs',
    slug: 'writable-ctes',
    sql: `-- move every post from one user to another
with moved as (
  delete from posts
  where user_id = :from_user::int
  returning title, body
)
insert into posts (user_id, title, body)
select :to_user::int, title, body from moved
returning id, title`,
  },
];
