alter table t drop index k;
alter table t
  modify column g point not null srid 3857,
  add spatial key k (g);
