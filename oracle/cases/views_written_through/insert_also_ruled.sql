insert into orders (id, note) values (:id, :note) returning id, note, state
