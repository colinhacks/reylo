create table users (id integer primary key, name varchar(100) not null, n bigint not null);
create view named as select id, name, n from users;
create view shouting as select id, upper(name) as name, n * 2 as twice from named;
