create table users (id integer primary key, name text not null, n integer not null check (n > 0));
create index users_n_idx on users (n);
