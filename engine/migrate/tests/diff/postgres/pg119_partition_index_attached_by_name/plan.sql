create table u (
  i integer
) partition by range (i);
create table up partition of u for values from (0) to (10);
create index ti on only t using btree (i);
alter index ti attach partition pz;
create index ua on up (i) with (fillfactor = 80);
create index ui on only u using btree (i);
create index uz on up (i) with (fillfactor = 70);
alter index ui attach partition uz;
