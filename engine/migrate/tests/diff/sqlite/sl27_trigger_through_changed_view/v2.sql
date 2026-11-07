create table users (id integer primary key, n integer not null check (n > 0));
create table log (id integer primary key, total integer);
create view totals as select count(*) as c, sum(n) as s from users;
create trigger users_log after insert on users begin insert into log (total) select c from totals; end;
