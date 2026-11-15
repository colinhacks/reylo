create table zz (id bigint primary key, n integer); create table aa (id bigint primary key, zz_id bigint references zz (id));
