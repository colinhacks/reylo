-- file 1 of 2
create function r() returns text language sql stable begin atomic select min(e) from h; end;
alter table p add constraint ck check (r() = r()) not valid;
alter table t add constraint ck check ((r() = r())) not valid;
alter table t validate constraint ck;
alter table t inherit p;
-- file 2 of 2
alter table p validate constraint ck;
