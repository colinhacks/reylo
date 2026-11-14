create macro valid(s) as length(s) > 0; create table t (id integer primary key, code varchar check (valid(code))); create view v as select valid('x') as ok;
