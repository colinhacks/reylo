create macro valid(s) as length(s) > 0; create table t (id integer primary key, code varchar check (valid(code))); create table u (id integer primary key, name varchar check (valid(name)));
