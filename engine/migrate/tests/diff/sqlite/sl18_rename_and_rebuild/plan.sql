create table new_users (id integer primary key, full_name text not null, n integer not null check (n > 0));
insert into new_users (id, full_name, n) select id, full_name, n from users;
drop table users;
alter table new_users rename to users;
create index users_n_idx on users (n);
-- users.full_name is renamed name, keeping its rows. If they are two columns, drop the one and add the other instead.
alter table users rename column full_name to name;
