-- loses data: users.code
alter table users drop column code;
create or replace view coded as select users.id as id, users.name as name from users;
