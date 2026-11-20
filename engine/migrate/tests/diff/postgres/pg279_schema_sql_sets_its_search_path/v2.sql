create schema app;
set search_path = app, public;
create sequence t2_c_seq;
create table t2 (c int not null default nextval('t2_c_seq'));
alter sequence t2_c_seq owned by t2.c;
create view v as select c from t2;
