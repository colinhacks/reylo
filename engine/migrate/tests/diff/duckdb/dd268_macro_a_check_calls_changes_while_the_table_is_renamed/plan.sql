-- loses data: t
drop table t;
drop macro valid;
create macro "valid" (s) as ((length(s) > 0));
create table t2 (id integer, code varchar, primary key (id), check ("valid"(code)));
