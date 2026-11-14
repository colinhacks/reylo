alter table ev alter column meta set data type struct(a integer, b varchar, c boolean);
alter table ev add column extra map(varchar, integer);
