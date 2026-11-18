drop view high_tasks;
alter type priority rename to priority__old;
create type priority as enum ('low', 'high', 'urgent');
alter table tasks alter column priority drop default;
alter table tasks alter column priority type priority using priority::text::priority;
alter table tasks alter column priority set default 'low'::priority;
drop type priority__old;
create view high_tasks as select id from tasks where priority in ('high', 'urgent');
