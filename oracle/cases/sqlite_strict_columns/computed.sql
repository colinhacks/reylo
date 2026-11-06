select i + 1 as plus, cast(i as text) as text_of, coalesce(n, 0) as filled, max(i, 1) as larger from s
