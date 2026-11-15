create macro valid(s) as length(s) > 0; create table t2 (id integer primary key, code varchar check (valid(code))); create table c2 (id integer primary key, t_id integer references t2 (id));
