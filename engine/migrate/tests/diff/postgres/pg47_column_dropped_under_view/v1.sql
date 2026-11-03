create table users (id integer primary key, name text not null, legacy text);
create view user_list as select id, name, legacy from users;
