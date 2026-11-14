create table users (id int primary key, full_name varchar(100) not null, n int not null, code text);
create table posts (id int primary key, user_id int not null);
