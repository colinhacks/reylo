create sequence s increment by 2; create table p (id integer primary key, n integer check (n > 0)); create table c (id bigint default nextval('s'), p_id integer references p (id), note varchar);
