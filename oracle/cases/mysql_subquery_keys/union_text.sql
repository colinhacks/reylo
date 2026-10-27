select * from (select email as k from users union select name from users) s where s.k = :k
