create sequence s increment by 5; create table keep (id bigint default nextval('s')); create table t2 (id bigint default nextval('s'), b varchar);
