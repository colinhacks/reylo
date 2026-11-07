create table new_users (id integer primary key, full_name text not null, n integer not null check (n > 0));
create table new_posts (id integer primary key, user_id integer not null references new_users (id) on delete cascade);
insert into new_users (id, full_name, n) select id, full_name, n from users;
insert into new_posts (id, user_id) select id, user_id from posts;
drop table posts;
drop table users;
alter table new_users rename to users;
alter table new_posts rename to posts;
