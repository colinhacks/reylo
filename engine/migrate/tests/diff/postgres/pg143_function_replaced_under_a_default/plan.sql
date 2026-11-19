-- file 1 of 2
drop view v;
alter table t drop constraint t_m_check;
alter table t alter column n drop default;
drop function f();
create function f() returns bigint language sql immutable as 'select 1';
alter table t alter column n set default f();
alter table t add constraint t_m_check check (m > f()) not valid;
create view v as select id, f() as k from t;
-- file 2 of 2
alter table t validate constraint t_m_check;
