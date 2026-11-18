create table readings (id integer not null, site text not null) partition by list (site);
create table readings_a partition of readings for values in ('a');
create table readings_b partition of readings for values in ('b');
alter table readings enable row level security;
alter table readings_a enable row level security;
alter table readings_b enable row level security;
alter table readings_b force row level security;
