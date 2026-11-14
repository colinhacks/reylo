-- loses data: users.name
drop view recent_titles;
drop view recent;
drop index users_name;
create temporary table reylo_copy_users as select * from users;
create temporary table reylo_copy_posts as select * from posts;
drop table posts;
drop table users;
create table users (id integer, email varchar not null, feeling mood, score integer default 0, primary key (id), unique (email));
create table posts (id integer, user_id integer, title varchar, body varchar, created date default current_date, primary key (id), foreign key (user_id) references users (id));
insert into users (id, email, feeling, score) select id, email, feeling, score from temp.main.reylo_copy_users;
insert into posts (id, user_id, title, body, created) select id, user_id, title, body, created from temp.main.reylo_copy_posts;
drop table temp.main.reylo_copy_users;
drop table temp.main.reylo_copy_posts;
comment on column users.email is 'login';
create index posts_title on posts(title);
create view recent as select p.id, p.title, u.email from (posts as p inner join users as u on ((u.id = p.user_id)));
create view recent_titles as select title from recent;
comment on view recent is 'latest';
