create table t (id integer default 1) partition by range (id);
create table p partition of t for values from (0) to (10);
alter table p alter column id drop default;
