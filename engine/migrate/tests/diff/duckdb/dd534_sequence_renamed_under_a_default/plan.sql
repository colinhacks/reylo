create sequence s2 start with 1 increment by 1 minvalue 1 maxvalue 9223372036854775807;
alter table t alter column id set default nextval('s2');
drop sequence s1;
