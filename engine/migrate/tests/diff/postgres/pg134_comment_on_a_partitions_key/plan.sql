comment on constraint events_leaf_pkey on events_leaf is null;
alter table events drop constraint events_pkey;
alter table events add constraint events_id_key primary key (id);
comment on constraint events_id_key on events is 'parent key';
comment on constraint events_leaf_pkey on events_leaf is 'child key';
