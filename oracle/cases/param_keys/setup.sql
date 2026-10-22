-- A parameter is a value, the same for every row, so one that is a whole sort, group or partition
-- key, bare or cast, sorts nothing, folds every row into one group or partition, or keeps one row:
-- each is an error at the parameter, and the rest of the statement is typed all the same. A key
-- that holds a parameter chooses a column with it and stays allowed, as do a position and LIMIT.
create table users (id int primary key, name text not null, created_at timestamptz not null default now());
