create table t (id integer primary key, s varchar, l VARCHAR[] generated always as (CAST(string_split(s, ',') AS VARCHAR[])) virtual);
