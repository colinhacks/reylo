create macro valid(s, n) as length(s) > n; create macro tm() as table select valid('x', 0) as ok; create table t (id integer primary key, code varchar check (valid(code, 0)));
