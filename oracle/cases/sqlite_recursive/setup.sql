-- A recursive WITH query's columns are a fixed point: its reads of itself are taken to be what
-- its first arm gives, then weakened to what the query gives under that, until nothing changes.
create table categories (id integer primary key, parent_id integer references categories (id), name text not null, note text);
create table users (id integer primary key, name text not null, email text unique);
create table posts (id integer primary key, user_id integer not null references users (id), title text not null, body text);
