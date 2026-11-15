-- loses data: t
drop table t;
drop macro valid;
create macro "valid" (s) as ((length(s) > 0));
create table u (id integer, label varchar, n integer, primary key (id), check ("valid"("label")));
