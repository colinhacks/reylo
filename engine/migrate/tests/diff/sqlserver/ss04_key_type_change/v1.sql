create table users (id int not null primary key, name nvarchar(40) null);
create table posts (id int not null primary key, user_id int not null foreign key references users (id), title nvarchar(40) not null default N'untitled');
create index posts_user on posts (user_id);
