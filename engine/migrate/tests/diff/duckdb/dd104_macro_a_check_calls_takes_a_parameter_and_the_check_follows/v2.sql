create macro valid(c, n) as length(c) > n; create table t (id integer primary key, code varchar check (valid(code, 0)));
