select (id, body) as pair, row(id) as single from everything where id = :id
