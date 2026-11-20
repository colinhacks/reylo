create table users (id int primary key, full_name varchar(100) not null, n int not null, code text);
create procedure touch() select 1;
create function twice(x int) returns int deterministic return x * 2;
create view names as select id, full_name from users;
grant select, insert on users to 'reader'@'%' with grant option;
grant update (full_name, code) on users to 'app_role';
grant select on names to 'app_role';
grant execute on procedure touch to 'reader'@'%';
grant execute, alter routine on function twice to 'app_role' with grant option;
