create table new_users (id integer primary key, full_name text not null, email text unique);
insert into new_users (id, full_name) select id, full_name from users;
drop table users;
alter table new_users rename to users;
