create macro ok(s) as length(s) > 0; create table t (id integer primary key, code varchar check (ok(code))); create table c (id integer primary key, t_id integer references t (id));
