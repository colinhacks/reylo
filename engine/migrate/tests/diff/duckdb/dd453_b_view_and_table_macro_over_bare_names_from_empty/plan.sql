create schema b;
create table b.t (y integer, x varchar);
create table t (x varchar);
create view b.w as select y from t;
create macro b.tm () as table (select x from t);
create view b.v as select * from b.tm();
