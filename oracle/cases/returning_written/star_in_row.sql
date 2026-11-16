update t set (id, note, label) = row(w.*, 'x') from w where t.id = 1 and w.a is not null returning note, label
