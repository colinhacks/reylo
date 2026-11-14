create macro x() as 1; create table u (id integer primary key, k integer default x(), n integer);
