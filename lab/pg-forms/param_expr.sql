select :x::int as x, coalesce(:y::nullable(text), 'd') as y
