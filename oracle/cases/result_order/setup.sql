-- The columns of a result its ORDER BY sorts on, each with its direction and where its NULLs sort, as far as its keys are columns of the result.
create table events (id int primary key, at timestamptz, score int, label text, kept int not null);
insert into events values (1, '2026-01-01', 5, 'a', 1), (2, null, null, 'b', 1), (3, '2026-01-02', 5, null, 2), (4, null, 1, 'A', 2);
create function events_listed() returns table(id int, label text) language sql stable as $$ select id, label from events $$;
