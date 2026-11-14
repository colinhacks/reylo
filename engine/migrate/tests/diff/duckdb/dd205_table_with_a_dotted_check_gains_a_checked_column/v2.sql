create table t (id integer primary key, meta struct(a integer), n integer check (n > 0), check (meta.a > 0)); create table c (id integer primary key, t_id integer references t (id));
