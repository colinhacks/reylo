create sequence s start with 100 increment by 1 minvalue 1 maxvalue 9223372036854775807;
alter table b.p2 alter column k drop default;
drop sequence b.s;
alter table b.p2 alter column k set default nextval('s');
