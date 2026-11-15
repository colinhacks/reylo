create type mood as enum ('a', 'b'); create sequence seq; create table t (id bigint default nextval('seq'), c mood);
