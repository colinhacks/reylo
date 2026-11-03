create table users (id integer primary key, full_name text not null);
create view user_codes as select id, full_name from users;
