alter table events replica identity using index events_id;
alter table messages replica identity full;
