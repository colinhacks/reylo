create schema rpt; create table t (id integer primary key, a integer check (a > 0)); create view rpt.v as select * from main.t;
