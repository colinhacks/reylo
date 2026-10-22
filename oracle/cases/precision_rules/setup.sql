-- Operators that return a value for every non-null operand, casts to arrays, a table read on both
-- sides of an outer join, scalar subqueries that always give one row, a view over a view, and the
-- set-returning functions whose rows each hold a value.
create table member (id int primary key, first_name text not null, last_name text not null, manager_id int references member (id), tags text[] not null default '{}');
create view v_inner as select id, first_name from member;
create view v_outer as select id, first_name from v_inner;
