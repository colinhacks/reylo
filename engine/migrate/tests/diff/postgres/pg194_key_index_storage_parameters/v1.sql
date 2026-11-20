create table t (id int, a int, b int, constraint t_pkey primary key (id) with (fillfactor = 70), constraint u unique (a), constraint w unique (b) include (id) with (fillfactor = 50) deferrable);
create table r (id int references t (id));
