alter table t drop column b;
alter table t alter column a type bigint;
alter table t add column b bigint generated always as (a * 2) stored;
