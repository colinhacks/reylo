create table new_users (id integer primary key, full_name text not null, n integer not null, code text, constraint n_pos check (n > 0));
insert into new_users (id, full_name, n, code) select id, full_name, n, code from users;
drop table users;
alter table new_users rename to users;
