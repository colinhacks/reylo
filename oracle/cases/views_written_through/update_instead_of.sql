update note_list set body = :body where id = :id returning body, kind
