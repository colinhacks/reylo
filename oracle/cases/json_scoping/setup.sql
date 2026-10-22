-- The names a statement's JSON is read by, scoped as Postgres scopes them: a WITH query's body
-- sees only the queries before it, a subquery's own WITH hides the statement's, a DML statement
-- writes its relation whatever a WITH query is named, a grouping set nulls what it groups by, and
-- a column with no alias goes by the name Postgres gives it.
create table member (id int primary key, first_name text not null);
create table team (id int primary key, name text not null, motto text);
-- Tables named like a WITH query that a name inside its own body, before it, or in a subquery
-- with a WITH of its own, reads instead.
create table snap (o int);
create table tally (x int not null);
create table x (o int);
