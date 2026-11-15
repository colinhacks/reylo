create table t (id integer primary key, s varchar check (CAST({'a': s} AS STRUCT(a VARCHAR)).a IS NOT NULL), n integer check (n > 0));
