-- loses data: ranges.hi
create table new_ranges (id integer primary key, lo integer not null);
insert into new_ranges (id, lo) select id, lo from ranges;
drop table ranges;
alter table new_ranges rename to ranges;
