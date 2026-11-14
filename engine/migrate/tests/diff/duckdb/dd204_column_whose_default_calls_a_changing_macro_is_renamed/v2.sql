create macro m() as 2; create table t (id integer primary key, ref_code integer default m(), other varchar);
