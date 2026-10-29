-- DuckDB's aggregates: a count is never NULL, any other aggregate is NULL over no rows, and over
-- a group's rows only where its argument is; grouping sets make a key NULL in a total's row.
create table orders (id integer primary key, customer varchar not null, region varchar, total decimal(10, 2) not null, note varchar);
