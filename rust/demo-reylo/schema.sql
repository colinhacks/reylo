-- The demo's schema, with the rows it reads. demo-codegen reads it too.
create type mood as enum ('ok', 'sad');
create table users (id int primary key, name text not null, nickname text, mood mood, joined timestamptz not null default now(), token uuid not null default gen_random_uuid(), balance numeric(10, 2) not null default 0);
create table posts (id serial primary key, author_id int not null references users (id), title text not null, tags text[] not null default '{}', status text not null default 'draft' check (status in ('draft', 'published')));
create table tasks (id serial primary key, state text not null default 'todo' check (state in ('todo', 'doing', 'done')));
insert into users (id, name, nickname, mood) values (1, 'ada', 'countess', 'ok'), (2, 'bob', null, 'sad');
insert into posts (author_id, title, tags) values (1, 'hello', '{intro}'), (1, 'again', '{intro,null}');
