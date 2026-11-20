select id, vector_extract(e) as e, vector_distance_l2(e, vector64(:q)) as d from wide where e is not null
