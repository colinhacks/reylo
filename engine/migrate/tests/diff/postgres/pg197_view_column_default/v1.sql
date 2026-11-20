create sequence s;
create table t (a int, b text);
create view v as select a, b from t;
create view w as select a, b from t;
alter view w alter column b set default 'x';
alter view w alter column a set default nextval('s');
