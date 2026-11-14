create macro y() as 1; create table p (id integer primary key, v integer); create table c (id integer primary key, p_id integer references p (id), w integer, check (w >= y()));
