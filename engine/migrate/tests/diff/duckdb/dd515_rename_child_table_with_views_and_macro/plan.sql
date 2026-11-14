-- loses data: posts
drop macro table posts_of;
drop view recent_titles;
drop view recent;
drop table posts;
create table articles (id integer, user_id integer, title varchar, body varchar, created date default current_date, primary key (id), foreign key (user_id) references users (id));
create index articles_title on articles(title);
create view recent as select p.id, p.title, u."name" from (articles as p inner join users as u on ((u.id = p.user_id)));
create view recent_titles as select title from recent;
create macro articles_of (uid) as table (select * from articles where (user_id = uid));
comment on view recent is 'latest';
