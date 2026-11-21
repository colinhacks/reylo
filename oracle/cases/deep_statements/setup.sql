-- Statements nested as deeply as the engine reads (engine/core/src/stack.rs: at most 1,000 levels of
-- Postgres's parse tree) and one level deeper. At the bound every pass of the analysis
-- runs over the whole tree and the key proves at most one row; past it the statement does not
-- parse locally, and its analysis is the server's describe alone: any number of rows, every column
-- nullable. The statements are generated; each file names its shape and depth.
create table users (id int primary key, name text not null, nick text);
