create table users (
  id integer primary key,
  name text not null,
  email text,
  role text not null default 'member' check (role in ('admin', 'member')),
  created_at text not null default current_timestamp
) strict;

create table posts (
  id integer primary key,
  user_id integer not null references users (id),
  title text not null,
  body text
) strict;
