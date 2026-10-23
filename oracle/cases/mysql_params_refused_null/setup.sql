-- A column that may hold NULL by its declaration may still refuse it through a CHECK (MySQL
-- 8.0.16 and later). Until 2026-10-21 a bare parameter written whole to such a column was typed
-- nullable, and MySQL refused the NULL (error 3819). A bare parameter written whole takes the
-- column's nullability, read from its NOT NULL and its enforced CHECKs, so each one here is a value
-- but `note`'s; one cast to `nullable(…)` may be null. The oracle fails a case whose NULL the database refuses
-- with a NOT NULL (1048) or a CHECK (3819) error.
create table accounts (id int primary key, email varchar(40) check (email is not null), nickname varchar(20) check (nickname is not null), note varchar(40));
insert into accounts values (1, 'ada@example.com', 'ada', null);
