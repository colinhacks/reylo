create table users (id int primary key, full_name varchar(100) not null, n int not null, code text);
create view user_n as select id, n from users;
