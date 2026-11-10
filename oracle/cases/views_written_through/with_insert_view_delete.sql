with made as (insert into published (id, title) values (:id, :title) returning *) delete from notes using made where notes.id = made.id returning made.published_at
