-- loses data: b.p2
drop table b.p2;
drop macro b.valid;
create table b.p3 (id integer, d varchar default "valid"('abc'), primary key (id));
