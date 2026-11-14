create table users (id integer primary key, email text not null, secret text);
grant select (id, email) on table users to col_reader;
grant update (email) on table users to col_reader;
