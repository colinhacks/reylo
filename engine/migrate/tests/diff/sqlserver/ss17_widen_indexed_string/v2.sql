create table tags (id int not null primary key, name nvarchar(60) not null);
create unique index tags_name on tags (name);
