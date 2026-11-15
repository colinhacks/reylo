create table t (id integer primary key, meta struct(a integer), check (meta.a > 0)); create table u (id integer primary key, k integer unique, s varchar check (s <> ''), n integer);
