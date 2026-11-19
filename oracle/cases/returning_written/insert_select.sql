insert into t (id, note) select id + 10, coalesce(note, '') from src returning note
