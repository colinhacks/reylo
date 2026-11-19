create table t (id int not null, c smallint) partition by range (id);
create table t_p0 partition of t for values from (1) to (4);
create index i on only t (c);
