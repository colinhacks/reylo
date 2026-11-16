create table users (id integer primary key, full_name text not null, n integer not null check (n > 0));
create table posts (id integer primary key, user_id integer not null references users (id) on delete cascade);
