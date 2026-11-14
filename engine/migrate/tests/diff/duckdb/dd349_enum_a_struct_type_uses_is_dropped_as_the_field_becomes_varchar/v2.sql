create type pair as struct(m varchar); create table t (id integer primary key, p pair);
