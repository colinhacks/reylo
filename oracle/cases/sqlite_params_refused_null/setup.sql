-- A column that may hold NULL by its declaration may still refuse it through a CHECK. Until
-- 2026-10-21 a bare parameter written whole to such a column was typed nullable, and SQLite refused
-- the NULL ("CHECK constraint failed"). A bare parameter written whole takes the column's
-- nullability, read from its NOT NULL and its CHECKs, so each one here is a value but `note`'s; one
-- cast to `nullable(…)` may be null. The oracle fails a case whose NULL the database refuses.
create table accounts (id integer primary key, email text check (email is not null), nickname text check (nickname is not null), note text);
insert into accounts values (1, 'ada@example.com', 'ada', null);
