create type mood as enum ('a', 'b', 'c'); create sequence seq increment by 2; create table t (id bigint default nextval('seq'), c mood);
