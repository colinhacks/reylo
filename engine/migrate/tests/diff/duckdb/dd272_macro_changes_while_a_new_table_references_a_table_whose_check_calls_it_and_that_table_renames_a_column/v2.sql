create macro valid(s) as length(s) > 0; create table t (id integer primary key, label varchar check (valid(label))); create table c (id integer primary key, t_id integer references t (id));
