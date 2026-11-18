create table "table" (uid uuid not null, column1 timestamp, column3 timestamp, "bool" boolean generated always as ((("table".column1 is null)) and (("table".column3 is null))) stored);
