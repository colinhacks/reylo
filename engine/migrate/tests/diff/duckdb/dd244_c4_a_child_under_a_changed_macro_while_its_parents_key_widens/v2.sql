create macro y() as 2; create table p (id bigint primary key, v integer); create table c (id integer primary key, p_id bigint references p (id), w integer, check (w >= y()));
