-- An INSERT of one VALUES row is planned as a select list: a set-returning call in it writes a
-- row per value it returns, and none for an empty set, and a LIMIT or an OFFSET may leave no row.
create table p (id int primary key, name text not null);
