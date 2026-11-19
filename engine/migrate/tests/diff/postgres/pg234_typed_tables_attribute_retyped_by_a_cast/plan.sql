-- loses data: t.a changes from text to numeric, which stores text that reads as a number as that number ('007' becomes 7)
-- loses data: ty.a changes from text to numeric, which stores text that reads as a number as that number ('007' becomes 7)
alter table t not of;
alter table t alter column a type numeric using a::numeric;
alter type ty alter attribute a type numeric;
alter table t of ty;
