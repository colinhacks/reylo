alter table c add constraint ck check ((id > 0)) not valid;
alter table c validate constraint ck;
alter table c inherit p;
