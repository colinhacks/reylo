create table new_users (id integer primary key, full_name text not null, n integer not null, n2 integer generated always as (n * 2) stored) strict;
insert into new_users (id, full_name, n) select id, full_name, n from users;
drop table users;
alter table new_users rename to users;
