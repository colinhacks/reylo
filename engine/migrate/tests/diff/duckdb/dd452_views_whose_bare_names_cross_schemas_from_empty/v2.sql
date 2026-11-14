create schema b; create table b.c (x varchar); create view b.a as select * from c; create view c as select * from b.a;
