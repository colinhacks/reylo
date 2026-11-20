select regexp_replace(id, 'z', 'x') as a, regexp_replace(score, '1', 'x') as b, regexp_replace(data, 'z', null) as c, math_floor(:x) as d from people
