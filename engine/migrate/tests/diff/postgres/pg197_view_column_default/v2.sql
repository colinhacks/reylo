create sequence s;
create table t (a int, b text);
create view v as select a, b from t;
alter view v alter column b set default 'x';
create view w as select a, b, a + 1 as c from t;
alter view w alter column b set default 'x';
alter view w alter column a set default nextval('s');
