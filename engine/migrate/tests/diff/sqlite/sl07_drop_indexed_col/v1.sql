create table users (id integer primary key, full_name text not null, n integer not null, code text);
create index users_code_idx on users (code);
