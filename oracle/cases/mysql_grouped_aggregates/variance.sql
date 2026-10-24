select p.user_id, var_samp(p.id) as v, stddev_pop(p.id) as s, group_concat(p.title) as g, bit_or(p.id) as b from posts p group by p.user_id
