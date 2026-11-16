-- The schema of the docs site's examples, with the rows the demo reads.
create type user_role as enum ('admin', 'member');
create table users (id serial primary key, name text not null, email text unique, role user_role not null default 'member', tags text[] not null default '{}', created_at timestamptz not null default now());
create table posts (id serial primary key, user_id int not null references users (id), title text not null, body text, published_at timestamptz);
insert into users (id, name, email, role) values (1, 'ada', 'ada@example.com', 'admin'), (2, 'bob', null, 'member');
insert into posts (user_id, title, published_at) values (1, 'hello', now()), (1, 'again', null);
