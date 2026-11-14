-- loses data: t
drop table t;
drop macro ok;
create macro ok (a) as ((a >= 0));
create table u (id integer, v integer, primary key (id), check (ok(v)));
