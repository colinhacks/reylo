update t set note = src.note from src where src.id = t.id returning t.note
