-- Each parameter appears once in a statement, so that one place types it:
-- a second use of a name, `:name` or `$n`, is an error at its place, and a value the
-- statement reads twice is selected once in a WITH. A parameter cast to `nullable(…)` there takes
-- NULL, which the WITH query's column carries to a test of it for NULL or a coalesce of it; one
-- cast to its type alone takes none, however the column is read; and the column pins a key as the
-- parameter does.
create table users (id int primary key, name text not null, email text, age int);
