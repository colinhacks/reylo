create table t (id integer primary key, "Score" bigint, d integer generated always as ("Score" * 2));
