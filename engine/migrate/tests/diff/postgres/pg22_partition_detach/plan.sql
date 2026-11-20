alter table events detach partition events_2;
alter table events_2 rename constraint events_id_not_null to events_2_id_not_null;
alter table events_2 rename constraint events_kind_not_null to events_2_kind_not_null;
