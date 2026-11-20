create table a (id int not null primary key);
create table b (id int not null primary key, a_id int null references a (id));
