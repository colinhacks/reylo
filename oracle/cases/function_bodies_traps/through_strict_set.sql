select s.v from (select null::text as a) t, lateral f_through_set(t.a) s
