insert into notes (body, tag) values (:body, :tag) returning id, body, tag, created
