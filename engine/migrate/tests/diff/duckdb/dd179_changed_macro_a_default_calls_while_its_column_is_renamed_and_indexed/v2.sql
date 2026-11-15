create macro x() as 2; create table u (id integer primary key, kk integer default x(), n integer); create index u_n on u (n);
