select b.id from nobody b having (select count(b.id) from posts limit 1) >= 0
