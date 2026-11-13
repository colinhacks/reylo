with i as (insert into inbox values (:id, :name) returning *) update notes set n = 1 from i where notes.id = i.id returning i.name
