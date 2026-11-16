insert into t (id, note, label) values ((row(90005, null)::pair).*, 'x') returning note, label
