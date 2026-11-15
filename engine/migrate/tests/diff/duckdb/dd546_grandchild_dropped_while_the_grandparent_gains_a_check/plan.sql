-- loses data: tags
drop table tags;
create temporary table reylo_copy_users as select * from users;
create temporary table reylo_copy_posts as select * from posts;
drop table posts;
drop table users;
create table users (id integer, email varchar, primary key (id), check ((email != '')));
create table posts (id integer, user_id integer, primary key (id), foreign key (user_id) references users (id));
insert into users (id, email) select id, email from temp.main.reylo_copy_users;
insert into posts (id, user_id) select id, user_id from temp.main.reylo_copy_posts;
drop table temp.main.reylo_copy_users;
drop table temp.main.reylo_copy_posts;
