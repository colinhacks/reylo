create sequence b.s start with 1 increment by 1 minvalue 1 maxvalue 9223372036854775807;
alter table b.p2 alter column d drop default;
drop sequence s;
alter table b.p2 alter column d set default nextval('s');
