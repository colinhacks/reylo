create extension if not exists pgcrypto;
create type post_status as enum ('draft', 'published', 'archived');

-- The signed-in user, as the request names them.
create function current_profile() returns uuid language sql stable as $$
  select nullif(current_setting('app.profile', true), '')::uuid
$$;

create function set_updated_at() returns trigger language plpgsql security definer set search_path = '' as $$
begin
  new.updated_at := now();
  return new;
end
$$;

create table profiles (
  id uuid primary key default gen_random_uuid(),
  name varchar(100) not null check (char_length(name) between 1 and 100),
  handle varchar(30) unique,
  created_at timestamptz not null default now(),
  updated_at timestamptz,
  deleted_at timestamptz
);
create trigger profiles_updated_at before update on profiles for each row execute function set_updated_at();

create table posts (
  id bigint generated always as identity primary key,
  author uuid not null references profiles on delete cascade,
  title text not null,
  body text not null default '',
  status post_status not null default 'draft',
  tags text[] not null default '{}',
  published_at timestamptz,
  constraint published_has_date check (status <> 'published' or published_at is not null)
);
create index posts_author on posts (author) where status <> 'archived';
create index posts_search on posts using gin (to_tsvector('english', title || ' ' || body));
create index posts_tags on posts using gin (tags);

alter table posts enable row level security;
create policy posts_read on posts for select using (status = 'published' or author = (select current_profile()));
create policy posts_write on posts for all using (author = (select current_profile())) with check (author = (select current_profile()));

create view published_posts with (security_invoker = true) as
  select p.id, p.title, a.name as author_name, p.published_at
  from posts p join profiles a on a.id = p.author
  where p.status = 'published' and a.deleted_at is null;

comment on table posts is 'What profiles write';
