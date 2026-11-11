insert into notes (body) select body || ' again' from notes where tag = :tag returning id, body
