create table posts (id int primary key, title text);
grant update (title) on posts to 'reader'@'%';
