select s.a from has_a, lateral strict_null(a) s
