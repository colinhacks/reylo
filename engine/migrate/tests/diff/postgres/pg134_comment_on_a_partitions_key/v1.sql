create table events (id integer, constraint events_pkey primary key (id)) partition by range (id);
create table events_leaf partition of events for values from (0) to (10);
comment on constraint events_pkey on events is 'parent key';
comment on constraint events_leaf_pkey on events_leaf is 'child key';
