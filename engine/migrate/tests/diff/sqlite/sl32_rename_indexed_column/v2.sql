create table users (id integer primary key, name text not null);
create index users_full_name_idx on users (name);
