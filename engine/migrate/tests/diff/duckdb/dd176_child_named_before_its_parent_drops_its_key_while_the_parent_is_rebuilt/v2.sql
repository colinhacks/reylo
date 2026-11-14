create table p (id integer primary key, n integer check (n > 0)); create table c (id integer primary key, p_id integer);
