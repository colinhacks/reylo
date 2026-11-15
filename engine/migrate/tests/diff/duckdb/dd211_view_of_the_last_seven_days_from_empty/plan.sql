create table events (id integer, seen timestamp);
create view recent as select * from events where (seen > (cast(now() as TIMESTAMP) - to_days(cast(trunc(cast(7 as DOUBLE)) as INTEGER))));
