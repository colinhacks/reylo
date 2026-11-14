create table users (id integer primary key, name text not null);
create view user_list as select id, name from users;
