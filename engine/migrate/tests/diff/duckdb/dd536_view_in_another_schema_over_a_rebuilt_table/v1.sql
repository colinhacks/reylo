create schema rpt; create table t (id integer primary key, a integer); create view rpt.v as select * from main.t;
