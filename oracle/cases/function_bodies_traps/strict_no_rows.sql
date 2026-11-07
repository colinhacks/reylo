select s.a from has_a, lateral strict_row(a) s
