create table t (id integer primary key, meta struct(a integer), check (meta.a > 0)); create table c (id integer primary key, t_id integer references t (id));
