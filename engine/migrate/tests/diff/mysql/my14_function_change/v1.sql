create table users (id int primary key, full_name varchar(100) not null, n int not null, code text);
create function bump(x int) returns int deterministic return x + 1;
