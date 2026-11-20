alter sequence seq1 as bigint increment by 1 minvalue 1 maxvalue 9223372036854775807 start with 1 cache 1 no cycle;
create sequence seq2 as bigint increment by 1 minvalue 1 maxvalue 9223372036854775807 start with 1 cache 1 no cycle;
alter table t2 add column c4 integer default nextval('seq2');
