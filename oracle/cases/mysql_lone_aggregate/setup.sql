-- An aggregate with no GROUP BY returns its one row over no rows too, with NULL in every column
-- outside an aggregate's call and in every subquery, whatever the column's NOT NULL, the WHERE or
-- a count says. An aggregate in a subquery's WHERE is the outer query's.
create table t (id int primary key, a int not null, s varchar(20) not null, b int);
create table o (id int primary key, t_id int not null);
