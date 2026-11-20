drop index i;
alter table t alter column c type character varying(10);
create index i on only t (c);
