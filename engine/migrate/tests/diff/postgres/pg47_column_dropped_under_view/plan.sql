-- loses data: users.legacy
drop view user_list;
alter table users drop column legacy;
create view user_list as select id, name from users;
