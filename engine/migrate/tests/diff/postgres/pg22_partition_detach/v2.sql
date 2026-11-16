create table events (id integer not null, kind integer not null, body text) partition by list (kind);
create table events_1 partition of events for values in (1);
-- kept as a table of its own, out of the partitioned table
create table events_2 (id integer not null, kind integer not null, body text);
