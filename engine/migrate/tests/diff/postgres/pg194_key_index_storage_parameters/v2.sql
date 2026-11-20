create table t (id int, a int, b int, constraint t_pkey primary key (id), constraint u unique (a) with (fillfactor = 10), constraint w unique (b) include (id) with (fillfactor = 60) deferrable);
create table r (id int references t (id));
