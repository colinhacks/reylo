create macro valid(s) as length(s) > 0; create table t2 (id integer primary key, code varchar check (valid(code)));
