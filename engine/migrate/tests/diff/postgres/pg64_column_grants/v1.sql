create table users (id integer primary key, email text not null, secret text);
grant select (secret) on table users to col_reader;
