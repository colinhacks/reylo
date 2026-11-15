-- loses data: posts.title
drop view recent_titles;
drop view recent;
drop index posts_title;
create temporary table reylo_copy_posts as select * from posts;
drop table posts;
create table posts (id integer, user_id integer, body varchar, created date default current_date, primary key (id), foreign key (user_id) references users (id));
insert into posts (id, user_id, body, created) select id, user_id, body, created from temp.main.reylo_copy_posts;
drop table temp.main.reylo_copy_posts;
create view recent as select p.id, u."name" from (posts as p inner join users as u on ((u.id = p.user_id)));
create view recent_titles as select "name" from recent;
comment on view recent is 'latest';
