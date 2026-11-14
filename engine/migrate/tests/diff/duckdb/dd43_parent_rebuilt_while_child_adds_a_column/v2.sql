create table zz (id bigint primary key, n integer check (n > 0)); create table aa (id bigint primary key, zz_id bigint references zz (id), note varchar);
