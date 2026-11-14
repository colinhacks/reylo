-- loses data: users.code
drop view user_codes;
alter table users drop column code;
create view user_codes as select id, full_name from users;
