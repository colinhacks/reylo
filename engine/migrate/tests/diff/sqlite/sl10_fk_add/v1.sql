create table users (id integer primary key, full_name text not null, n integer not null, code text);
create table posts (id integer primary key, user_id integer not null);
