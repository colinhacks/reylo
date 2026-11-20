alter table h add column c integer;
alter table h add constraint ck check ((c > 0)) not valid;
alter table h validate constraint ck;
alter table h inherit p;
