// The schema `reylo play` gives PGlite when a project names no database and no schema file: the
// users and posts of the docs' examples, so that the playground opens on tables to query.
export const SAMPLE_SCHEMA = `create type user_role as enum ('admin', 'member');

create table users (
  id serial primary key,
  name text not null,
  email text unique,
  role user_role not null default 'member',
  tags text[] not null default '{}',
  created_at timestamptz not null default now()
);

create table posts (
  id serial primary key,
  user_id integer not null references users (id),
  title text not null,
  body text,
  published_at timestamptz
);`;
