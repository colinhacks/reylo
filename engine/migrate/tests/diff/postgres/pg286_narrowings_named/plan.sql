-- loses data: t.a changes from numeric(65,30) to double precision, which keeps about 15 significant digits
-- loses data: t.b changes from double precision to integer, which rounds to whole numbers
-- loses data: t.c changes from numeric(10,2) to numeric(10,0), which rounds to whole numbers
-- loses data: t.d changes from real to integer, which rounds to whole numbers
-- loses data: t.e changes from integer to real, which keeps about 6 significant digits
-- loses data: t.f changes from bigint to double precision, which keeps about 15 significant digits
-- loses data: t.g changes from text to timestamp without time zone, which stores text that reads as a time as that time ('2026-1-2' becomes 2026-01-02)
-- loses data: t.h changes from text to jsonb, which keeps the JSON's values, not its duplicate keys, key order or spacing
-- loses data: t.i changes from json to jsonb, which keeps the JSON's values, not its duplicate keys, key order or spacing
-- loses data: t.j changes from interval to interval day, which keeps only the day of each interval
-- loses data: t.k changes from timestamp with time zone to timestamp without time zone, which keeps the local time, not the moment
-- loses data: t.l changes from time(6) without time zone to time(0) without time zone, which rounds to whole seconds
-- loses data: t.m changes from timestamp(6) without time zone to timestamp(2) without time zone, which rounds to 2 decimal places of a second
-- loses data: t.n changes from double precision to real, which keeps about 6 significant digits
-- loses data: t.o changes from numeric(5,2) to integer, which rounds to whole numbers
alter table t alter column a type double precision;
alter table t alter column b type integer;
alter table t alter column c type numeric(10,0);
alter table t alter column d type integer;
alter table t alter column e type real;
alter table t alter column f type double precision;
alter table t alter column g type timestamp without time zone using g::timestamp without time zone;
alter table t alter column h type jsonb using h::jsonb;
alter table t alter column i type jsonb;
alter table t alter column j type interval day using j::text::interval day;
alter table t alter column k type timestamp without time zone;
alter table t alter column l type time(0) without time zone;
alter table t alter column m type timestamp(2) without time zone;
alter table t alter column n type real;
alter table t alter column o type integer;
