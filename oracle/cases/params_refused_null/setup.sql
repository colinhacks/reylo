-- A column the catalog lets hold NULL may still refuse it: through a domain declared NOT NULL, a
-- domain that checks VALUE IS NOT NULL, or a CHECK that tests the column IS NOT NULL. Until
-- 2026-10-21 a bare parameter written whole to such a column was typed nullable, and the database
-- refused the NULL the caller was allowed to pass ("domain required_email does not allow null
-- values", a check violation). A bare parameter written whole, by VALUES, by the SELECT that feeds
-- an INSERT, by SET or by a row value, takes the column's nullability, read from its NOT NULL, its
-- domains and its CHECKs, so each one here is a value but `note`'s; a cast one is a value; one cast
-- to `nullable(…)` may be null. The oracle binds NULL to every parameter typed nullable and fails the
-- case where the database refuses it with a NOT NULL or a CHECK violation.
create domain required_email as text not null;
create domain present_text as text check (value is not null);
create table accounts (id int primary key, email required_email, nickname text check (nickname is not null), note text, tag present_text);
insert into accounts values (1, 'ada@example.com', 'ada', null, 'x');
