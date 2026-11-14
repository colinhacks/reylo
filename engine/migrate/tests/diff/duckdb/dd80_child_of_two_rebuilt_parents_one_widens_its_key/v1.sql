create table aa (id integer primary key, n integer); create table bb (id integer primary key); create table cc (id integer, aa_id integer references aa (id), bb_id integer references bb (id));
