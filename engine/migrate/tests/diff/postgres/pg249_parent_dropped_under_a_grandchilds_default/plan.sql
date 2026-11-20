-- loses data: t
alter table g no inherit h;
alter table h no inherit t;
alter table h drop column if exists id;
alter table g alter column id drop default;
alter table g alter column id drop not null;
drop table t;
alter table g inherit h;
