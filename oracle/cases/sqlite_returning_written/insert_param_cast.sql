insert into t (id, note) values (:id, cast(:note as text)) returning note
