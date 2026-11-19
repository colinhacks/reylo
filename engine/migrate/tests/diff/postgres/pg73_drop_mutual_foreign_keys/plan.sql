-- loses data: b
-- loses data: a
alter table a drop constraint a_b_fkey;
drop table b;
drop table a;
