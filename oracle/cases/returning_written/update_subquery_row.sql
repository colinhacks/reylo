update t set (note, n) = (select note, 1 from src where src.id = t.id) returning note, n
