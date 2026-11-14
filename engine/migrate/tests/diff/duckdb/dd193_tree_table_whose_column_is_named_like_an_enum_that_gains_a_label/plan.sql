drop type kind;
create type kind as enum('a', 'b', 'c');
alter table things alter column k set data type kind;
