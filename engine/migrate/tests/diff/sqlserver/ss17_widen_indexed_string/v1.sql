create table tags (id int not null primary key, name varchar(20) not null);
create unique index tags_name on tags (name);
