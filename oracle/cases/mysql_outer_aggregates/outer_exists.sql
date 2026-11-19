select b.id, exists (select max(b.id) from posts limit 1) as e from nobody b
