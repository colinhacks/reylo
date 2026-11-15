-- loses data: t.a changes from time with time zone to time, which keeps the time of day, not its offset
-- loses data: t.d changes from double to boolean, which keeps only whether each number is zero (5 becomes true)
-- loses data: t.n changes from integer to boolean, which keeps only whether each number is zero (5 becomes true)
-- loses data: t.s changes from varchar to blob, which reads each backslash escape as a byte ('\x41' becomes A)
alter table t alter column a set data type time;
alter table t alter column n set data type boolean;
alter table t alter column s set data type blob;
alter table t alter column d set data type boolean;
