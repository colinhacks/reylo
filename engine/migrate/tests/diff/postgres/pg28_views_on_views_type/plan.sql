drop view shouting;
drop view named;
alter table users alter column n type bigint;
alter table users alter column name type character varying(100);
create view named as select id, name, n from users;
create view shouting as select id, upper(name) as name, n * 2 as twice from named;
