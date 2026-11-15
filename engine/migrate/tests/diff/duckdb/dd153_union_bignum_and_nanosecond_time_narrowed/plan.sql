-- loses data: t.b changes from bignum to double, which keeps about 15 significant digits
-- loses data: t.ns changes from time_ns to time, which rounds to 6 decimal places of a second
-- loses data: t.u changes from union(n double, s varchar) to union(n integer, s varchar), which rounds to whole numbers, in its member n
alter table t alter column u set data type union(n integer, s varchar);
alter table t alter column b set data type double;
alter table t alter column ns set data type time;
