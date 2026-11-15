create macro valid(s) as s <> ''; create table t (id integer primary key, code varchar check (valid(code))); create table c (id integer primary key, t_id integer references t (id));
