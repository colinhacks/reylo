create type ty as (a int, b text);
create table t of ty;
create table u of ty (a with options not null default 1, primary key (a));
create type ty2 as (x int, y text);
create table w of ty2 (x with options default 5, y with options default 'y');
create table n of ty2;
