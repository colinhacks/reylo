create table p (id integer primary key, n integer); create table c (id integer primary key, p_id integer references p (id));
