drop index t_a;
alter table t add column active boolean not null default true;
create index t_a on t(a);
