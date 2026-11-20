-- loses data: people.name
alter table people drop column name;
alter table people add column age integer;
create table new_users (id bigint not null, username text not null default '', age integer);
insert into new_users (rowid, id, username, age) select rowid, id, username, age from users;
drop table users;
alter table new_users rename to users;
-- users.username is renamed user_name, keeping its rows. If they are two columns, drop the one and add the other instead.
alter table users rename column username to user_name;
