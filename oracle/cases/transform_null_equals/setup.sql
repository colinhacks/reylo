-- Under transform_null_equals, which a database or a role may set, the server reads `x = NULL` as
-- `x IS NULL`: the NULL keys all match it, so it pins no key and leaves the column nullable.
set transform_null_equals = on;
create table nd (id int primary key, x int unique, y text);
insert into nd values (1, null, 'a'), (2, null, 'b'), (3, 3, 'c');
