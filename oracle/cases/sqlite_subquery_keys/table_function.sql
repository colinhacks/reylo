select * from (select p.id, j.value from posts p, json_each(:ids) j) s where s.id = :id
