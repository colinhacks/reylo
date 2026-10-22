-- A column read from a derived table or a WITH query is as NULL as the engine finds the table's
-- own column, through whatever reads it: a count in a scalar subquery is never NULL inside the
-- table, so it is never NULL outside it, but on the null-extended side of an outer join, under a
-- ROLLUP, or in a column the table computes, NULL comes back. The server merges a derived table
-- into the query that reads it and flags a merged expression NOT NULL even where an outer join
-- null-extends it, which the engine reads past by preparing the statement materialized.
create table users (id int auto_increment primary key, name text not null, email text);
create table posts (id int auto_increment primary key, user_id int not null, title text not null);
