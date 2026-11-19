create sequence s;
create table t (id int, c int default nextval('s')) with (fillfactor = 80);
alter sequence s owned by t.c;
