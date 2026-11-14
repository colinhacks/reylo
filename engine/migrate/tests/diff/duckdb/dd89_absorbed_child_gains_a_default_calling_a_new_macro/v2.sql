create macro m() as 42; create table p (id integer primary key, n integer check (n > 0)); create table c (id integer, p_id integer references p (id), k integer default m());
