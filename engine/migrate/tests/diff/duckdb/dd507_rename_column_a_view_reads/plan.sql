drop view recent_titles;
drop view recent;
drop index posts_title;
create temporary table reylo_copy_posts as select * from posts;
drop table posts;
create table posts (id integer, user_id integer, heading varchar, body varchar, created date default current_date, primary key (id), foreign key (user_id) references users (id));
insert into posts (id, user_id, heading, body, created) select id, user_id, title, body, created from temp.main.reylo_copy_posts;
drop table temp.main.reylo_copy_posts;
-- posts.title is renamed heading, keeping its rows. If they are two columns, drop the one and add the other instead.
create index posts_title on posts(heading);
create view recent as select p.id, p.heading, u."name" from (posts as p inner join users as u on ((u.id = p.user_id)));
create view recent_titles as select heading from recent;
comment on view recent is 'latest';
