create view names as select users.id as id, users.full_name as full_name from users;
create function twice(x int) returns int deterministic return x * 2;
grant alter routine, execute on function twice to `app_role`@`%` with grant option;
grant execute on procedure touch to `reader`@`%`;
grant update (code), update (full_name) on users to `app_role`@`%`;
grant insert on users to `reader`@`%` with grant option;
grant select on names to `app_role`@`%`;
