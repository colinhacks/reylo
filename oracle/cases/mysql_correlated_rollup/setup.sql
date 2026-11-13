-- A subquery grouped WITH ROLLUP beside the columns of the query around it. MySQL 8.4.13 answers
-- an INT column of a table as a BIGINT through a temporary table where the subquery reads it in
-- its select list beside an aggregate and the query around it groups by it, though the prepare
-- names the table; each statement but `full` and `no_order` leaves out one part of that.
create table users (id int primary key, name varchar(20) not null, email varchar(40), flag boolean not null default true, score decimal(10,2), ratio double, born date, seen datetime, at time, doc json, raw varbinary(8), kind enum('a','b'), bits bit(8), y year, big bigint, tags set('x','y'), n int not null default 1);
create table posts (id int primary key, user_id int not null, title varchar(20) not null, tag varchar(20), score int, n int not null default 1, key (user_id));
