-- status is not null with no default, so the copy fails on a table that has rows: give it a default, or add it nullable, fill it, then make it not null
create table new_users (id integer primary key, full_name text not null, status text not null);
insert into new_users (id, full_name) select id, full_name from users;
drop table users;
alter table new_users rename to users;
