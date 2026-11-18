create table t (i integer) partition by range (i);
create table p partition of t for values from (0) to (10);
create index pa on p (i);
create index pz on p (i);
create index ti on only t (i);
alter index ti attach partition pz;
