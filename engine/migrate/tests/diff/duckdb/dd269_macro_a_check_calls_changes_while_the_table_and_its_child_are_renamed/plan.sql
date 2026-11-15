-- loses data: c
-- loses data: t
drop table c;
drop table t;
drop macro valid;
create macro "valid" (s) as ((length(s) > 0));
create table t2 (id integer, code varchar, primary key (id), check ("valid"(code)));
create table c2 (id integer, t_id integer, primary key (id), foreign key (t_id) references t2 (id));
