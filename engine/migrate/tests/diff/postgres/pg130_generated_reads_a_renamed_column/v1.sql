create table "table" (uid uuid not null, column1 timestamp, column2 timestamp, "bool" boolean generated always as ((("table".column1 is null)) and (("table".column2 is null))) stored);
