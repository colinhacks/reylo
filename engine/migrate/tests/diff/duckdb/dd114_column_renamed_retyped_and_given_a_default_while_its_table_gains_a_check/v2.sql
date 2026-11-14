create table t (id integer primary key, quantity bigint default 1 check (quantity >= 0), note varchar);
