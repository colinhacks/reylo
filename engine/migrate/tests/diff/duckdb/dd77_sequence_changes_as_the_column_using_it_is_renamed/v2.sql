create sequence s increment by 2; create table t (key bigint default nextval('s'), name varchar);
