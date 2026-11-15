create table users (id integer primary key, full_name text not null, n integer not null);
create index users_n_idx on users (n);
