create table users (id bigint not null primary key, name nvarchar(40) null);
create table posts (id int not null primary key, user_id bigint not null foreign key references users (id), title nvarchar(40) not null default N'untitled');
create index posts_user on posts (user_id);
