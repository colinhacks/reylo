create macro valid(s, n) as length(s) > n; create table t (id integer primary key, code varchar check (valid(code, 0))); create view v as select valid('x', 0) as ok;
