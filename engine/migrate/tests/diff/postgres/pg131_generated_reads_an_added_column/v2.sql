create table t (id integer primary key, a_total integer generated always as (z_qty * 2) stored, z_qty integer default 1);
