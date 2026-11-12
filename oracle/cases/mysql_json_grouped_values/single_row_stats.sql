select (select json_object('vs', var_samp(p.n), 'ss', stddev_samp(p.n), 'sd', std(p.n), 'v', variance(p.n), 'ba', bit_and(p.n)) from posts p group by p.id order by p.id limit 1) as o
