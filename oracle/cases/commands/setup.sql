-- A statement of a kind the engine does not read (DDL, a session setting, EXPLAIN, CALL) is a
-- command: typed from the server's describe alone, with no rows where it returns none and every
-- column nullable where it returns some. Postgres binds a parameter only in a command it analyzes
-- when it prepares it (EXPLAIN, CALL, CREATE TABLE ... AS). A CREATE of a table the schema has
-- prepares, and fails when it runs.
create table users (id int primary key, name text not null);
create materialized view user_names as select name from users;
create procedure touch(n int) language sql as $$ select n $$;
create procedure count_users(out n bigint) language sql as $$ select count(*) from users $$;
