drop trigger users_log;
drop view totals;
create table new_users (id integer primary key, n integer not null check (n > 0));
insert into new_users (id, n) select id, n from users;
drop table users;
alter table new_users rename to users;
create view totals as select count(*) as c, sum(n) as s from users;
create trigger users_log after insert on users begin insert into log (total) select c from totals; end;
