create table kept (id integer primary key);
create table t (i integer) partition by range (i);
create table p partition of t for values from (0) to (10);
create index pi on p (i) with (fillfactor = 70);
create index ti on t (i);
comment on index pi is 'kept';
