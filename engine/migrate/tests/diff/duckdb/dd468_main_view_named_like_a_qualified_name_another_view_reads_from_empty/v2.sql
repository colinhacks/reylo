create schema b; create schema c; create table c.n (x integer); create view b.v as select * from c.n; create view n as select * from b.v;
