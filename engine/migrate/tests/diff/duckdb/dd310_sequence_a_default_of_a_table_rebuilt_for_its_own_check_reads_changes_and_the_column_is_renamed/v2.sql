create sequence s start 10; create table t (id integer primary key, ticket bigint default nextval('s'), n integer check (n > 0));
