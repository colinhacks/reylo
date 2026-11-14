create table users (id integer primary key, full_name text not null);
create index users_full_name_idx on users (full_name);
