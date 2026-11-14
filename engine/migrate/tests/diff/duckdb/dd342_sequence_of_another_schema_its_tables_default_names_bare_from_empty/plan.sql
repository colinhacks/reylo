create schema a;
create sequence a.s start with 500 increment by 1 minvalue 1 maxvalue 9223372036854775807;
create sequence s start with 100 increment by 1 minvalue 1 maxvalue 9223372036854775807;
create table a.t (id bigint default nextval('s'), n integer, primary key (id));
