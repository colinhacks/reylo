create type ty as (a int, b text);
create table t (a int, b text);
create table u of ty;
create type ty2 as (x int);
create table w of ty2 (x with options default 5);
