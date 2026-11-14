create temporary table reylo_copy_ev as select * from ev;
drop table ev;
create table ev (id uuid default gen_random_uuid(), data json, amount decimal(12,2), happened timestamp with time zone default now(), tags integer[], primary key (id), check (json_valid("data")));
insert into ev (id, data, amount, happened, tags) select id, data, amount, happened, tags from temp.main.reylo_copy_ev;
drop table temp.main.reylo_copy_ev;
