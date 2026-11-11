select * from orders o join users u using (id) where note = :n
