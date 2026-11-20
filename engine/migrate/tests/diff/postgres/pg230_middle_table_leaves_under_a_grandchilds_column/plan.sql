alter table g no inherit h;
alter table h no inherit t;
alter table h drop column if exists id;
alter table g alter column id drop not null;
alter table g inherit h;
