-- A statement of a kind the engine does not read (DDL, a PRAGMA, EXPLAIN) is a command: typed
-- from SQLite's prepare alone, with no rows where it returns none and every column of no known
-- type and nullable where it returns some. A CREATE of a table the schema has fails the prepare.
create table users (id integer primary key, name text not null);
