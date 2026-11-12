select json_object('vs', var_samp(p.n), 'ss', stddev_samp(p.n), 'sd', std(p.n)) as o from posts p group by p.id
