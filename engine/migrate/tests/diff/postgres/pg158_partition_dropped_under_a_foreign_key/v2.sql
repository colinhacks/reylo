create table t (id integer not null, primary key (id)) partition by range (id);
create table t_p0 partition of t for values from (1) to (3);
create table c (id integer primary key, parent integer references t (id));
