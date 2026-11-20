-- loses data: t.a changes from text to integer, which stores text that reads as a number as that number ('007' becomes 7)
-- loses data: ty.a changes from text to integer, which stores text that reads as a number as that number ('007' becomes 7)
alter table t not of;
alter table t alter column a drop default;
alter table t alter column a type integer using a::integer;
alter type ty alter attribute a type integer;
alter table t of ty;
alter table t alter column a set default 1;
