-- A row value of a table's type is held to the table's NOT NULL only where it is one of the
-- table's stored rows: a whole row the statement reads (`d` of `from docs d`), through a view, a
-- derived table or a WITH query too, or one a scalar or ARRAY subquery returns as its one target.
-- A row the statement builds (`row(…)::docs`), one another table's column holds, and one a
-- function returns met no constraint of the table; and a bare name a column of the statement's
-- tables takes reads that column, not a row.
create table docs (id int primary key, title text not null, note text);
insert into docs values (1, 'a', null), (2, 'b', 'n');
create table holders (id int primary key, h docs);
insert into holders values (1, row(null, null, 'n')::docs), (2, row(2, 'b', null)::docs);
create table shadows (id int primary key, c docs);
insert into shadows values (1, row(null, null, null)::docs);
create function make_doc() returns docs language sql stable as $$ select row(null, null, null)::docs $$;
create view doc_rows as select d from docs d;
-- A column named like its table, of a composite type named like the table in another schema: a
-- bare name reads the column, which may be NULL, and not the table's row.
create schema other;
create type other.wr as (a int);
create type other.wr6 as (z int);
create table wr (id int primary key, wr other.wr);
create table wr6 (id int primary key, x other.wr6);
insert into wr values (1, null);
insert into wr6 values (1, null);
