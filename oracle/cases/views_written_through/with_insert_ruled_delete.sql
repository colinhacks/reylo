with i as (insert into inbox values (:id, :name) returning *) delete from notes using i where notes.id = i.id returning i.name, notes.body
