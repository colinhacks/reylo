drop index te;
alter table t rename column a to aa;
alter table t alter column n set data type bigint;
alter table t alter column s set not null;
alter table t alter column e set default 'd';
create index te on t(e);
comment on index te is 'by e';
-- t.a is renamed aa, keeping its rows. If they are two columns, drop the one and add the other instead.
