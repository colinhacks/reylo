create table t (i integer) partition by range (i);
create table p partition of t for values from (0) to (10);
create index pa on p (i);
create index pz on p (i);
create index qa on p (i) with (fillfactor = 80);
create table w (i integer) partition by range (i);
create table wp partition of w for values from (0) to (10);
create index wa on wp (i desc);
create index wz on wp (i);
