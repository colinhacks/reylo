create table new_posts (id integer primary key, user_id integer not null references users (id));
insert into new_posts (id, user_id) select id, user_id from posts;
drop table posts;
alter table new_posts rename to posts;
