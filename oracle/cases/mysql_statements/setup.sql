-- What is a statement the engine types: one statement. A SELECT, INSERT, UPDATE, DELETE, REPLACE,
-- VALUES or TABLE is read; any other is a command, typed from the prepare alone, with no rows
-- where it returns none and every column nullable where it returns some. Two statements, and a
-- CALL, whose result sets the prepare does not describe, are errors and are never run; what the
-- server refuses to prepare is an error with the server's message; and a statement the engine's
-- own parser cannot read keeps its types from the server and loses the row count and the JSON
-- shapes. MySQL runs the text of a `/*! … */` comment as SQL.
create table users (id int primary key, name varchar(20) not null, email varchar(40) unique, doc json, fulltext key (name));
create table log (id int primary key auto_increment, line varchar(40) not null);
create procedure touch() begin insert into log (line) values ('touched'); end;
create function next_line() returns int deterministic reads sql data return (select count(*) from log);
create trigger users_log after insert on users for each row insert into log (line) values (concat('user ', new.id));
