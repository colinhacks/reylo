drop view user_n;
alter table users alter column n type bigint using n::bigint;
create view user_n as select id, n from users;
