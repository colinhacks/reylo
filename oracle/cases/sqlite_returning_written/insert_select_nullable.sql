insert into t (id, note) select id + 10, note from src returning note
