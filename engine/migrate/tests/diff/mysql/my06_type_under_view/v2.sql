create table users (id int primary key, full_name varchar(100) not null, n bigint not null, code text);
create view user_n as select id, n from users;
