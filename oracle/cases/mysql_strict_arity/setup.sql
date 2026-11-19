-- A built-in is NULL for a NULL argument only at the numbers of arguments, and the positions,
-- measured so: a later patch of `json_merge_patch` replaces what a NULL made of the ones before
-- it, and `json_contains_path` with 'one' stops at the first path it finds, before it reads a
-- NULL after it. So a WHERE that tests such a call passes rows where `n` is NULL, and `n` stays
-- nullable; where the NULL is the last patch, or the first path, it rejects them. `left(n, 0)`
-- is '' wherever `n` is not NULL, so the path is `$`.
create table docs (id int primary key, n int);
insert into docs values (1, null), (2, 5);
