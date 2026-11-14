create table p (id integer primary key, n integer); create table z (id integer primary key, p_id integer references p (id));
