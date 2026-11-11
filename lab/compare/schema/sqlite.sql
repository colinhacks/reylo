create table users (id integer primary key, email text not null unique, name text not null, nickname text);
create table posts (id integer primary key, author_id integer not null references users (id), title text not null, score integer, created_at text not null default current_timestamp);
