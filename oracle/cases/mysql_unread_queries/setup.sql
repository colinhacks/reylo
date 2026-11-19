-- The server merges a derived table into the query that reads it, and a merged expression keeps
-- its NOT NULL flag on the null-extended side of an outer join: `n` is flagged NOT NULL, and every
-- row holds NULL in it. Only the engine's tree shows where such a table stands, so a query whose
-- tree it sets aside (`||`, which the server reads as OR under the default `sql_mode`; a hint that
-- sets a variable) or cannot read (`sounds like`) claims no column non-null.
create table users (id int primary key, name varchar(50) not null);
