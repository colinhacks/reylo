create table users (id int primary key, full_name varchar(100) not null, n int not null, code text, n2 int generated always as (n * 3) stored);
