create table t (id integer primary key, meta struct(a integer)); create table u (id integer primary key, p integer references t (id), k integer unique, s varchar check (s <> ''));
