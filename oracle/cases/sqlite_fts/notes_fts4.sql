select rowid as id, offsets(notes) as o, matchinfo(notes) as m, snippet(notes) as s from notes where notes match :q
