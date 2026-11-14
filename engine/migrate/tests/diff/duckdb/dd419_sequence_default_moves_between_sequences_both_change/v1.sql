create sequence s1; create sequence s2; create table t (id bigint default nextval('s1'), n integer); create table u (id bigint default nextval('s2'));
