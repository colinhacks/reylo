create table users (id integer primary key, full_name text not null, n integer not null, code text);
create view user_names as select id, full_name, n from users;
