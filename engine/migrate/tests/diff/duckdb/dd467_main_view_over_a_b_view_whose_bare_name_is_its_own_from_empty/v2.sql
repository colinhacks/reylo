create schema b; create table b.c (x varchar); create view b.z as select * from c; create view c as select * from b.z;
