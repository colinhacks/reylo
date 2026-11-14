create table users (id integer primary key, full_name text not null);
create table posts (id integer primary key, user_id integer references users (id), title text);
create view post_titles as select p.title, u.full_name from posts p join users u on u.id = p.user_id;
