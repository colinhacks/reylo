create sequence s1 increment by 2; create sequence s2 increment by 3; create table t (id bigint default nextval('s2'), n integer); create table u (id bigint default nextval('s1'));
