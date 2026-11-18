create type priority as enum ('low', 'high', 'urgent');
create table tasks (id integer primary key, priority priority default 'low');
create view high_tasks as select id from tasks where priority in ('high', 'urgent');
