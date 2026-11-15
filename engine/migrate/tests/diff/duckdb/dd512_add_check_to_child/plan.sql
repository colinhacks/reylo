create temporary table reylo_copy_posts as select * from posts;
drop table posts;
create table posts (id integer, user_id integer, title varchar, body varchar, created date default current_date, primary key (id), foreign key (user_id) references users (id), check ((length(body) < 1000)));
insert into posts (id, user_id, title, body, created) select id, user_id, title, body, created from temp.main.reylo_copy_posts;
drop table temp.main.reylo_copy_posts;
create index posts_title on posts(title);
