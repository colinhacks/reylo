-- loses data: t.l changes from timestamp[] to date[], which keeps only the date, in each element
-- loses data: t.s changes from struct(a integer, b integer) to struct(a integer), which drops its field b
alter table t alter column s set data type struct(a integer);
alter table t alter column l set data type date[];
