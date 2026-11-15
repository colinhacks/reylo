-- loses data: users.code
drop index users_code_idx;
alter table users drop column code;
