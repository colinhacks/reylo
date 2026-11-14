create table events (id integer, seen timestamp); create view recent as select * from events where seen > now()::timestamp - interval 7 day;
