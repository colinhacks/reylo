create macro m() as 1; create table t (id integer primary key, code integer default m(), other varchar);
