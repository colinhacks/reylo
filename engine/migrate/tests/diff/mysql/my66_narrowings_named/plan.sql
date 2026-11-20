-- loses data: t.a changes from decimal(65,30) to double, which keeps about 15 significant digits
-- loses data: t.b changes from bigint to double, which keeps about 15 significant digits
-- loses data: t.c changes from double to int, which rounds to whole numbers
-- loses data: t.e changes from datetime(6) to datetime(3), which rounds to 3 decimal places of a second
-- loses data: t.f changes from datetime to date, which keeps only the date
-- loses data: t.g changes from text to json, which keeps the JSON's values, not its duplicate keys, key order or spacing
-- loses data: t.h changes from varchar(10) to int, which stores text that reads as a number as that number ('007' becomes 7)
alter table t
  modify column a double,
  modify column b double,
  modify column c int,
  modify column d timestamp null,
  modify column e datetime(3),
  modify column f date,
  modify column g json,
  modify column h int,
  modify column j bigint;
