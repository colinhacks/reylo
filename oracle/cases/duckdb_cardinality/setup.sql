-- How many rows a DuckDB statement returns: a key held equal to one value, an aggregate with no
-- grouping, a LIMIT, VALUES, and what a write returns.
create table accounts (id integer primary key, handle varchar not null unique, region varchar not null, tier varchar, unique (region, tier));
create table events (account_id integer not null references accounts (id), seen_at timestamp not null, kind varchar not null);
