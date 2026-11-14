create table t (id integer primary key, seen varchar default CAST(now() AS TIMESTAMP WITH TIME ZONE)::VARCHAR);
