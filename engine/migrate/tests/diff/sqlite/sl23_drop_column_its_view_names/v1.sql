create table users (id integer primary key, full_name text not null, code text);
create view user_codes as select id, full_name, code from users;
