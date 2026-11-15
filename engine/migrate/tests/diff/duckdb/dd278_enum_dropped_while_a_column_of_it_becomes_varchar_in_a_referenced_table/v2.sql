create table t (id integer primary key, m varchar); create table c (id integer primary key, t_id integer references t (id));
