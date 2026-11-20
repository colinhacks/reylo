create table 'bigdata' ('data' integer);
create table new_t (c numeric default '10.123');
insert into new_t (rowid, c) select rowid, c from t;
drop table t;
alter table new_t rename to t;
