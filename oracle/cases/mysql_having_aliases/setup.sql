-- In a HAVING, MySQL reads a bare name as the select list's alias before the FROM's column of that
-- name, so a parameter compared with one takes the alias's type, not the column's: `s as e` is
-- a string, `count(*) as b8` a number.
create table t (id int primary key, s varchar(20) not null, e enum('x','y') not null, b8 bit(8) not null);
insert into t values (1, 'abc', 'x', 1), (2, 'd', 'y', 2);
