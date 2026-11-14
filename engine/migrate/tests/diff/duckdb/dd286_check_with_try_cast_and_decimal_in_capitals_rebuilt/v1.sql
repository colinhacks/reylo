create table t (id integer primary key, s varchar check (TRY_CAST(s AS DECIMAL(10,2)) IS NOT NULL), n integer);
