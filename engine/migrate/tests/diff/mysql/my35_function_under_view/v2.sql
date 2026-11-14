create table users (id int primary key, n int); create function bump(x int) returns int deterministic return x + 2; create view bumped as select id, bump(n) as m from users;
