-- "table".column2 is renamed column3, keeping its rows. If they are two columns, write instead:
--   alter table "table" drop column column2;
--   alter table "table" add column column3 timestamp without time zone;
alter table "table" rename column column2 to column3;
alter table "table" alter column bool set expression as ("table".column1 is null and "table".column3 is null);
