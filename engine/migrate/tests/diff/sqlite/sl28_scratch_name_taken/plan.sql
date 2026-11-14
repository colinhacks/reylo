create table new_users_2 (id integer primary key, n integer not null check (n > 0));
insert into new_users_2 (id, n) select id, n from users;
drop table users;
alter table new_users_2 rename to users;
