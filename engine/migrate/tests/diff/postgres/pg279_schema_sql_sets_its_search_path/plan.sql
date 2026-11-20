create sequence app.t2_c_seq as bigint increment by 1 minvalue 1 maxvalue 9223372036854775807 start with 1 cache 1 no cycle;
create table app.t2 (
  c integer default nextval('app.t2_c_seq'::regclass) not null
);
alter sequence app.t2_c_seq owned by app.t2.c;
create view app.v as
select c
   from app.t2;
