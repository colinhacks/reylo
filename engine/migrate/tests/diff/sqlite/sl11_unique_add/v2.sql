create table users (id integer primary key, full_name text not null, n integer not null, code text);
create unique index users_code_key on users (code);
